//! Répertoire des contacts d'un serveur, avec les notes du staff. Accès :
//! `contact.notes` (gradés par défaut).

use super::{
    Access, MaybeViewer, Viewer,
    pages::{self, Layout},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::contacts::{
        Kind, Relation,
        service::{self, Contact, Filter, Note},
    },
    modules::permissions::Perm,
    util::time::{format_date_label, format_day_label},
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

#[derive(Debug, Default, Deserialize)]
pub struct ContactQuery {
    relation: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    q: Option<String>,
}

impl ContactQuery {
    /// Valeurs invalides ignorées plutôt que refusées.
    fn filter(&self) -> Filter {
        Filter {
            relation: self.relation.as_deref().and_then(Relation::parse),
            kind: self.kind.as_deref().and_then(Kind::parse),
            query: self.q.as_deref().map(str::trim).filter(|q| !q.is_empty()).map(|q| q.chars().take(100).collect()),
        }
    }
}

/// Classes du badge de relation (jetons sémantiques du thème).
fn badge_class(relation: Relation) -> &'static str {
    match relation {
        Relation::Allie => "bg-staff-soft text-staff",
        Relation::Neutre => "bg-system-soft text-system",
        Relation::Hostile => "bg-danger-soft text-danger",
    }
}

struct Choice {
    value: &'static str,
    label: &'static str,
    active: bool,
}

struct Figure {
    label: &'static str,
    value: usize,
    tone: &'static str,
}

struct NoteRow {
    author: String,
    date: String,
    body: String,
}

struct ContactRow {
    name: String,
    kind: String,
    relation: String,
    badge: &'static str,
    phone: Option<String>,
    description: Option<String>,
    updated: String,
    notes: Vec<NoteRow>,
}

#[derive(Template)]
#[template(path = "contacts.html")]
struct ContactsPage {
    layout: Layout,
    guild_id: String,
    guild_name: String,
    q: String,
    filtered: bool,
    relations: Vec<Choice>,
    kinds: Vec<Choice>,
    figures: Vec<Figure>,
    contacts: Vec<ContactRow>,
}

fn name_of(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    user_id.parse().ok().and_then(|id| members::display_name(ctx, guild_id, id)).unwrap_or_else(|| user_id.to_owned())
}

fn choices<T: Copy + PartialEq>(
    all: &[T],
    current: Option<T>,
    value: impl Fn(T) -> &'static str,
    label: impl Fn(T) -> &'static str,
    any: &'static str,
) -> Vec<Choice> {
    std::iter::once(Choice { value: "", label: any, active: current.is_none() })
        .chain(all.iter().map(|item| Choice {
            value: value(*item),
            label: label(*item),
            active: current == Some(*item),
        }))
        .collect()
}

fn figures(contacts: &[Contact]) -> Vec<Figure> {
    let count = |relation: Relation| contacts.iter().filter(|c| c.relation() == relation).count();
    vec![
        Figure { label: "Contacts", value: contacts.len(), tone: "text-ink" },
        Figure { label: "Alliés", value: count(Relation::Allie), tone: "text-staff" },
        Figure { label: "Neutres", value: count(Relation::Neutre), tone: "text-system" },
        Figure { label: "Hostiles", value: count(Relation::Hostile), tone: "text-danger" },
    ]
}

fn contact_row(contact: &Contact, notes: Vec<NoteRow>) -> ContactRow {
    let (kind, relation) = (contact.kind(), contact.relation());
    ContactRow {
        name: contact.name.clone(),
        kind: format!("{} {}", kind.emoji(), kind.label()),
        relation: relation.label().to_owned(),
        badge: badge_class(relation),
        phone: contact.phone.clone(),
        description: contact.description.clone(),
        updated: format_day_label(contact.updated_at),
        notes,
    }
}

/// `/contacts` : le serveur de la personne, ou l'accueil s'il en a plusieurs.
pub async fn index(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri) -> Response {
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, uri.path()).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    match access.guilds_with(Perm::ContactNotes).as_slice() {
        [] => pages::forbidden(&ctx, &viewer, &access),
        [(id, _)] => Redirect::to(&format!("/stats/{id}/contacts")).into_response(),
        _ => Redirect::to("/").into_response(),
    }
}

async fn authorize(
    ctx: &Ctx,
    viewer: MaybeViewer,
    uri: &Uri,
    guild_id: u64,
) -> Result<(Viewer, Access, Id<GuildMarker>, String), Box<Response>> {
    let path = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let (viewer, access) = pages::require_viewer(ctx, viewer, path).await?;
    let Some(guild_id) = Id::<GuildMarker>::new_checked(guild_id) else {
        return Err(Box::new(pages::not_found().await));
    };
    if !access.can(guild_id, Perm::ContactNotes) {
        return Err(Box::new(pages::forbidden(ctx, &viewer, &access)));
    }
    let name = access.guild_name(guild_id);
    Ok((viewer, access, guild_id, name))
}

pub async fn page(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<ContactQuery>,
) -> Response {
    let (viewer, access, guild_id, guild_name) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let filter = query.filter();
    let guild = guild_id.to_string();
    let data = async {
        let contacts = service::list(&ctx.db, &guild, &filter).await?;
        let notes = service::notes_by_contact(&ctx.db, &guild).await?;
        anyhow::Ok((contacts, notes))
    }
    .await;
    let (contacts, mut notes) = match data {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(?error, "lecture des contacts");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Contacts indisponibles.");
        }
    };
    let rows = contacts
        .iter()
        .map(|contact| {
            let notes = notes
                .remove(&contact.id)
                .unwrap_or_default()
                .into_iter()
                .map(|note: Note| NoteRow {
                    author: name_of(&ctx, guild_id, &note.author_id),
                    date: format_date_label(note.created_at),
                    body: note.body,
                })
                .collect();
            contact_row(contact, notes)
        })
        .collect();

    pages::render(&ContactsPage {
        layout: Layout::new(&ctx, format!("Contacts · {guild_name}"), Some(&viewer), &access, "/contacts"),
        guild_id: guild,
        guild_name,
        q: filter.query.clone().unwrap_or_default(),
        filtered: filter.relation.is_some() || filter.kind.is_some() || filter.query.is_some(),
        relations: choices(&Relation::ALL, filter.relation, Relation::as_str, Relation::plural_label, "Toutes"),
        kinds: choices(&Kind::ALL, filter.kind, Kind::as_str, Kind::label, "Tous"),
        figures: figures(&contacts),
        contacts: rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::time::DbTime;

    fn contact(name: &str, kind: Kind, relation: Relation, phone: Option<&str>, description: Option<&str>) -> Contact {
        Contact {
            id: name.into(),
            name: name.into(),
            kind: kind.as_str().into(),
            relation: relation.as_str().into(),
            phone: phone.map(str::to_owned),
            description: description.map(str::to_owned),
            created_by_id: "1".into(),
            created_at: DbTime::now(),
            updated_at: DbTime::now(),
        }
    }

    fn note(author: &str, date: &str, body: &str) -> NoteRow {
        NoteRow { author: author.into(), date: date.into(), body: body.into() }
    }

    #[test]
    fn parses_filters() {
        let query =
            ContactQuery { relation: Some("hostile".into()), kind: Some("x".into()), q: Some("  port ".into()) };
        let filter = query.filter();
        assert_eq!(filter.relation, Some(Relation::Hostile));
        assert_eq!(filter.kind, None);
        assert_eq!(filter.query.as_deref(), Some("port"));
        assert!(ContactQuery::default().filter().query.is_none());
    }

    #[test]
    fn renders_contacts_page() {
        let contacts = vec![
            contact(
                "Garage Benny's",
                Kind::Entreprise,
                Relation::Allie,
                Some("555-0142"),
                Some("Pièces et réparations à prix d'ami. Demander Benny en personne."),
            ),
            contact("Vagos", Kind::Groupe, Relation::Allie, None, Some("Accord de passage sur Jamestown Street.")),
            contact(
                "Tony « le Rat »",
                Kind::Personne,
                Relation::Neutre,
                Some("555-8812"),
                Some("Indic au port, vend ses infos au plus offrant."),
            ),
            contact("Ballas", Kind::Groupe, Relation::Hostile, None, Some("Gang de Grove Street.")),
        ];
        let notes = [
            vec![note("Elodie", "16/10/2026 21:40", "Remise de 10 % négociée sur les pièces moteur.")],
            vec![],
            vec![
                note("Marco", "17/10/2026 23:05", "A vendu notre planque aux Ballas : ne plus rien lui confier."),
                note("Elodie", "12/10/2026 18:20", "Fiable pour les horaires des cargos."),
            ],
            vec![note("Sam", "15/10/2026 02:10", "Fusillade près du garage, deux blessés chez nous.")],
        ];
        let page = ContactsPage {
            layout: Layout::bare("Contacts"),
            guild_id: "1".into(),
            guild_name: "Serveur RP".into(),
            q: String::new(),
            filtered: false,
            relations: choices(&Relation::ALL, None, Relation::as_str, Relation::plural_label, "Toutes"),
            kinds: choices(&Kind::ALL, None, Kind::as_str, Kind::label, "Tous"),
            figures: figures(&contacts),
            contacts: contacts.iter().zip(notes).map(|(c, n)| contact_row(c, n)).collect(),
        };
        let html = page.render().unwrap();
        assert!(html.contains("bg-danger-soft text-danger") && html.contains("bg-staff-soft text-staff"));
        assert!(html.contains("Tony « le Rat »") && html.contains("ne plus rien lui confier"));
        assert!(html.contains("2 notes"));
        if let Ok(path) = std::env::var("CONTACTS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, super::super::ASSET_VERSION),
                &format!("<style>{}</style>", super::super::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
