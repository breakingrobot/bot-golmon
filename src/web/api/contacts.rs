//! Répertoire des contacts : lecture, ajout, modification, suppression, notes.
//! Chaque écriture met à jour l'annuaire publié sur Discord et le journal,
//! comme les commandes `/contact`.

use super::{
    ApiError, ApiQuery, ApiResult, JsonBody, MemberRef, Members, Session, created, double_option, iso_db, json,
    no_content,
};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        contacts::{
            Kind, NAME_TAKEN, Relation,
            commands::{added_details, changes, deleted_details, note_details},
            optional_text,
            service::{self, Contact as Row, Filter, NewContact, Note},
            validate, validate_note,
        },
        permissions::Perm,
    },
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::{IntoParams, ToSchema};

/// Notes renvoyées au plus pour un contact.
const NOTES_LIMIT: i64 = 500;
const NOT_FOUND: &str = "Contact introuvable.";

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub name: String,
    #[schema(value_type = super::openapi::ContactKind)]
    pub kind: &'static str,
    #[schema(value_type = super::openapi::Relation)]
    pub relation: &'static str,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub description: Option<String>,
    pub created_by: MemberRef,
    pub created_at: String,
    pub updated_at: String,
    #[schema(required = true)]
    pub notes_count: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContactNote {
    pub id: String,
    pub author: MemberRef,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ContactCounts)]
pub struct Counts {
    pub allie: usize,
    pub neutre: usize,
    pub hostile: usize,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContactsResponse {
    pub counts: Counts,
    pub items: Vec<Contact>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContactDetail {
    pub contact: Contact,
    pub notes: Vec<ContactNote>,
}

pub fn contact(members: &Members, row: &Row, notes_count: Option<i64>) -> Contact {
    Contact {
        id: row.id.clone(),
        name: row.name.clone(),
        kind: row.kind().as_str(),
        relation: row.relation().as_str(),
        phone: row.phone.clone(),
        description: row.description.clone(),
        created_by: members.get(&row.created_by_id),
        created_at: iso_db(row.created_at),
        updated_at: iso_db(row.updated_at),
        notes_count,
    }
}

fn note(members: &Members, note: &Note) -> ContactNote {
    ContactNote {
        id: note.id.clone(),
        author: members.get(&note.author_id),
        body: note.body.clone(),
        created_at: iso_db(note.created_at),
    }
}

async fn note_counts(db: &SqlitePool, guild_id: &str) -> anyhow::Result<HashMap<String, i64>> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as(r#"SELECT "contactId", COUNT(*) FROM "ContactNote" WHERE "guildId" = ? GROUP BY "contactId""#)
            .bind(guild_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().collect())
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ContactQuery {
    /// `ALLIE`, `NEUTRE` ou `HOSTILE`.
    relation: Option<String>,
    /// `GROUPE`, `ENTREPRISE` ou `PERSONNE`.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Texte cherché dans le nom ou la description.
    q: Option<String>,
}

/// Répertoire filtré (`contact.voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/contacts", tag = "contacts", operation_id = "getContacts",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ContactQuery),
    responses(
        (status = 200, description = "Contacts", body = ContactsResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn list(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<ContactQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactVoir)?;
    let gid = guild_id.to_string();
    let filter = Filter {
        relation: query.relation.as_deref().and_then(Relation::parse),
        kind: query.kind.as_deref().and_then(Kind::parse),
        query: query.q.filter(|q| !q.trim().is_empty()),
    };
    // Compteurs : tout le répertoire, sans les filtres.
    let all = service::list(&ctx.db, &gid, &Filter::default()).await?;
    let mut counts = Counts::default();
    for row in &all {
        match row.relation() {
            Relation::Allie => counts.allie += 1,
            Relation::Neutre => counts.neutre += 1,
            Relation::Hostile => counts.hostile += 1,
        }
    }
    let rows = if filter.relation.is_none() && filter.kind.is_none() && filter.query.is_none() {
        all
    } else {
        service::list(&ctx.db, &gid, &filter).await?
    };
    let notes = if session.can(guild_id, Perm::ContactNotes) { Some(note_counts(&ctx.db, &gid).await?) } else { None };
    let members = Members::new(&ctx, guild_id);
    let items = rows
        .iter()
        .map(|row| contact(&members, row, notes.as_ref().map(|n| n.get(&row.id).copied().unwrap_or(0))))
        .collect();
    Ok(json(&ContactsResponse { counts, items }))
}

async fn find(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> ApiResult<Row> {
    let row = service::resolve(&ctx.db, &guild_id.to_string(), id).await?;
    row.filter(|row| row.id == id.trim()).ok_or_else(|| ApiError::not_found(NOT_FOUND))
}

async fn detail_of(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, row: &Row) -> ApiResult<ContactDetail> {
    let members = Members::new(ctx, guild_id);
    if !session.can(guild_id, Perm::ContactNotes) {
        return Ok(ContactDetail { contact: contact(&members, row, None), notes: Vec::new() });
    }
    let (notes, total) = service::notes(&ctx.db, &guild_id.to_string(), &row.id, NOTES_LIMIT).await?;
    Ok(ContactDetail {
        contact: contact(&members, row, Some(total)),
        notes: notes.iter().map(|n| note(&members, n)).collect(),
    })
}

/// Fiche d'un contact (`contact.voir` ; notes si `contact.notes`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/contacts/{id}", tag = "contacts", operation_id = "getContact",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du contact")),
    responses(
        (status = 200, description = "Contact", body = ContactDetail),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn detail(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactVoir)?;
    let row = find(&ctx, guild_id, &id).await?;
    Ok(json(&detail_of(&ctx, &session, guild_id, &row).await?))
}

/// Limites : nom 80, téléphone 30, description 500 caractères.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContactInput {
    pub name: String,
    #[schema(value_type = super::openapi::ContactKind)]
    pub kind: String,
    #[schema(value_type = super::openapi::Relation)]
    pub relation: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Saisie d'un nouveau contact, validée comme `/contact ajouter`.
pub fn new_contact(input: ContactInput, created_by_id: String) -> Result<NewContact, String> {
    let name = input.name.trim().to_owned();
    let kind = Kind::parse(&input.kind).ok_or("Type invalide : GROUPE, ENTREPRISE ou PERSONNE.")?;
    let relation = Relation::parse(&input.relation).ok_or("Relation invalide : ALLIE, NEUTRE ou HOSTILE.")?;
    let phone = optional_text(input.phone);
    let description = optional_text(input.description);
    validate(&name, phone.as_deref(), description.as_deref())?;
    Ok(NewContact { name, kind, relation, phone, description, created_by_id })
}

/// Met à jour l'annuaire du salon sans faire échouer l'action déjà faite.
async fn refresh_directory(ctx: &Ctx, guild: &str) {
    if let Err(error) = service::sync_directory(ctx, guild).await {
        tracing::warn!(?error, "mise à jour du répertoire des contacts impossible");
    }
}

async fn journal(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, action: Action, details: String) {
    let actor = session.actor_name(ctx, guild_id);
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, Entry::new(action, None, details)).await;
}

/// Ajoute un contact (`contact.ajouter`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/contacts", tag = "contacts", operation_id = "createContact",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = ContactInput,
    responses(
        (status = 201, description = "Contact ajouté", body = Contact),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn create(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(input): JsonBody<ContactInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactAjouter)?;
    let gid = guild_id.to_string();
    let new = new_contact(input, session.viewer.id.to_string()).map_err(ApiError::invalid)?;
    let Some(row) = service::add(&ctx.db, &gid, new).await? else {
        return Err(ApiError::invalid(NAME_TAKEN));
    };
    refresh_directory(&ctx, &gid).await;
    journal(&ctx, &session, guild_id, Action::ContactAdd, added_details(&row)).await;
    let notes = session.can(guild_id, Perm::ContactNotes).then_some(0);
    Ok(created(&contact(&Members::new(&ctx, guild_id), &row, notes)))
}

/// `Partial<ContactInput>` : champ absent = inchangé ; `phone`/`description` à `null` = effacé.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ContactPatch {
    pub name: Option<String>,
    #[schema(value_type = Option<super::openapi::ContactKind>)]
    pub kind: Option<String>,
    #[schema(value_type = Option<super::openapi::Relation>)]
    pub relation: Option<String>,
    #[serde(deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub phone: Option<Option<String>>,
    #[serde(deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
}

/// Applique une modification, validée comme `/contact modifier`.
pub fn apply_patch(before: &Row, patch: ContactPatch) -> Result<Row, String> {
    let mut after = before.clone();
    if let Some(name) = patch.name {
        after.name = name.trim().to_owned();
    }
    if let Some(kind) = patch.kind {
        after.kind = Kind::parse(&kind).ok_or("Type invalide : GROUPE, ENTREPRISE ou PERSONNE.")?.as_str().to_owned();
    }
    if let Some(relation) = patch.relation {
        after.relation =
            Relation::parse(&relation).ok_or("Relation invalide : ALLIE, NEUTRE ou HOSTILE.")?.as_str().to_owned();
    }
    if let Some(phone) = patch.phone {
        after.phone = optional_text(phone);
    }
    if let Some(description) = patch.description {
        after.description = optional_text(description);
    }
    validate(&after.name, after.phone.as_deref(), after.description.as_deref())?;
    Ok(after)
}

/// Modifie un contact (`contact.modifier`) ; `phone`/`description` à `null` = effacer.
#[utoipa::path(
    patch, path = "/api/guilds/{guild}/contacts/{id}", tag = "contacts", operation_id = "updateContact",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du contact")),
    request_body = ContactPatch,
    responses(
        (status = 200, description = "Contact modifié", body = Contact),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn update(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
    JsonBody(patch): JsonBody<ContactPatch>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactModifier)?;
    let gid = guild_id.to_string();
    let before = find(&ctx, guild_id, &id).await?;
    let after = apply_patch(&before, patch).map_err(ApiError::invalid)?;
    let changes = changes(&before, &after);
    // Rien de changé : pas d'écriture, pas de journal.
    if !changes.is_empty() {
        if !service::save(&ctx.db, &gid, &after).await? {
            return Err(ApiError::invalid(NAME_TAKEN));
        }
        refresh_directory(&ctx, &gid).await;
        let details = format!("**{}** — {}", after.name, changes.join(" · "));
        journal(&ctx, &session, guild_id, Action::ContactUpdate, details).await;
    }
    let row = find(&ctx, guild_id, &before.id).await?;
    Ok(json(&detail_of(&ctx, &session, guild_id, &row).await?.contact))
}

/// Supprime un contact et ses notes (`contact.modifier`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/contacts/{id}", tag = "contacts", operation_id = "deleteContact",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du contact")),
    responses(
        (status = 204, description = "Supprimé"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn remove(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactModifier)?;
    let gid = guild_id.to_string();
    let row = find(&ctx, guild_id, &id).await?;
    let (_, notes) = service::notes(&ctx.db, &gid, &row.id, 0).await?;
    service::delete(&ctx.db, &gid, &row.id).await?;
    refresh_directory(&ctx, &gid).await;
    journal(&ctx, &session, guild_id, Action::ContactDelete, deleted_details(&row, notes)).await;
    Ok(no_content())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NoteInput {
    /// 1000 caractères au plus.
    body: String,
}

/// Ajoute une note du staff (`contact.notes`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/contacts/{id}/notes", tag = "contacts", operation_id = "createContactNote",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du contact")),
    request_body = NoteInput,
    responses(
        (status = 201, description = "Note ajoutée", body = ContactNote),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn add_note(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
    JsonBody(input): JsonBody<NoteInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::ContactNotes)?;
    let gid = guild_id.to_string();
    let row = find(&ctx, guild_id, &id).await?;
    let body = input.body.trim().to_owned();
    validate_note(&body).map_err(ApiError::invalid)?;
    let saved = service::add_note(&ctx.db, &gid, &row.id, &session.viewer.id.to_string(), &body).await?;
    journal(&ctx, &session, guild_id, Action::ContactNote, note_details(&row, &body)).await;
    Ok(created(&note(&Members::new(&ctx, guild_id), &saved)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, kind: &str, relation: &str, phone: Option<&str>) -> ContactInput {
        ContactInput {
            name: name.into(),
            kind: kind.into(),
            relation: relation.into(),
            phone: phone.map(Into::into),
            description: None,
        }
    }

    #[test]
    fn validates_new_contacts() {
        let ok = new_contact(input(" Les Ballas ", "GROUPE", "HOSTILE", Some(" - ")), "1".into()).unwrap();
        assert_eq!(
            (ok.name.as_str(), ok.kind, ok.relation, ok.phone),
            ("Les Ballas", Kind::Groupe, Relation::Hostile, None)
        );
        assert!(new_contact(input("  ", "GROUPE", "ALLIE", None), "1".into()).err().unwrap().contains("vide"));
        assert!(new_contact(input(&"a".repeat(81), "GROUPE", "ALLIE", None), "1".into()).is_err());
        assert!(new_contact(input("x", "GANG", "ALLIE", None), "1".into()).err().unwrap().contains("Type"));
        assert!(new_contact(input("x", "GROUPE", "AMI", None), "1".into()).err().unwrap().contains("Relation"));
        assert!(new_contact(input("x", "GROUPE", "ALLIE", Some(&"5".repeat(31))), "1".into()).is_err());
    }

    #[test]
    fn patch_distinguishes_absent_and_null() {
        let patch: ContactPatch = serde_json::from_str(r#"{"relation":"ALLIE","phone":null}"#).unwrap();
        assert_eq!(patch.phone, Some(None));
        assert_eq!(patch.description, None);
        let before = Row {
            id: "c1".into(),
            name: "Tony".into(),
            kind: "PERSONNE".into(),
            relation: "NEUTRE".into(),
            phone: Some("555".into()),
            description: Some("Indic".into()),
            created_by_id: "1".into(),
            created_at: crate::util::time::DbTime::now(),
            updated_at: crate::util::time::DbTime::now(),
        };
        let after = apply_patch(&before, patch).unwrap();
        assert_eq!((after.relation.as_str(), after.phone.as_deref()), ("ALLIE", None));
        assert_eq!(after.description.as_deref(), Some("Indic"));
        let bad: ContactPatch = serde_json::from_str(r#"{"name":"  "}"#).unwrap();
        assert!(apply_patch(&before, bad).is_err());
    }
}
