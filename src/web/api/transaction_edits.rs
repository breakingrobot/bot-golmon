//! Écritures des transactions depuis le site (`transaction.utiliser`) :
//! création, statut, objets, annulation. Comme `/transaction`, chaque écriture
//! publie ou met à jour la fiche Discord et le tableau de bord ; elle est aussi
//! inscrite au journal du staff. Depuis le site, la fiche d'une nouvelle
//! transaction est publiée dans le salon du tableau de bord.

use super::{
    ApiError, ApiResult, JsonBody, Members, Session, created, iso_db, json,
    transactions::{Item, Transaction},
};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        permissions::Perm,
        transactions::{
            commands::CLOSED,
            model::{Status, Transaction as Row, type_label},
            service::{self, NewTransaction},
        },
    },
    util::{format::format_price, time::DbTime},
};
use anyhow::Result;
use axum::extract::{Path, State};
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::ToSchema;

const NOT_FOUND: &str = "Transaction introuvable.";
const GROUPE_MAX: usize = 100;
const NAME_MAX: usize = 100;
const ITEMS_MAX: usize = 25;
const PRICE_MAX: f64 = 1_000_000_000_000.0;
const QUANTITY_MAX: i64 = 1_000_000;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionItemInput {
    /// 100 caractères au plus.
    pub nom: String,
    /// 1 par défaut.
    #[serde(default)]
    pub quantite: Option<i64>,
    #[serde(default)]
    pub prix_unitaire: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewTransactionInput {
    /// Groupe concerné, 100 caractères au plus.
    pub groupe: String,
    #[serde(rename = "type")]
    #[schema(value_type = super::openapi::TransactionType)]
    pub kind: String,
    /// Prix total.
    pub prix: f64,
    /// 25 objets au plus.
    #[serde(default)]
    pub items: Vec<TransactionItemInput>,
    /// Note libre, 500 caractères au plus (au-delà, coupée).
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TransactionStatusInput {
    /// Tout statut sauf `ANNULEE` (annulation : route dédiée).
    #[schema(value_type = super::openapi::TransactionStatus)]
    pub status: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckedItem {
    pub nom: String,
    pub quantite: i64,
    pub prix_unitaire: Option<f64>,
}

fn check_price(value: f64, label: &str) -> Result<f64, String> {
    if value.is_finite() && (0.0..=PRICE_MAX).contains(&value) { Ok(value) } else { Err(format!("{label} invalide.")) }
}

/// Objet validé comme le formulaire « Ajouter un objet ».
pub fn check_item(input: &TransactionItemInput) -> Result<CheckedItem, String> {
    let nom = input.nom.trim().to_owned();
    if nom.is_empty() {
        return Err("Indique le nom de l'objet.".to_owned());
    }
    if nom.chars().count() > NAME_MAX {
        return Err(format!("Le nom de l'objet dépasse {NAME_MAX} caractères."));
    }
    let quantite = input.quantite.unwrap_or(1);
    if !(1..=QUANTITY_MAX).contains(&quantite) {
        return Err("Quantité invalide.".to_owned());
    }
    let prix_unitaire = input.prix_unitaire.map(|p| check_price(p, "Prix unitaire")).transpose()?;
    Ok(CheckedItem { nom, quantite, prix_unitaire })
}

/// Transaction validée : groupe, type (`ACHAT` / `VENTE`), prix et objets.
pub fn check_transaction(input: &NewTransactionInput) -> Result<(String, &'static str, f64, Vec<CheckedItem>), String> {
    let groupe = input.groupe.trim().to_owned();
    if groupe.is_empty() {
        return Err("Indique le groupe concerné.".to_owned());
    }
    if groupe.chars().count() > GROUPE_MAX {
        return Err(format!("Le groupe dépasse {GROUPE_MAX} caractères."));
    }
    let kind = match input.kind.trim() {
        "ACHAT" => "ACHAT",
        "VENTE" => "VENTE",
        _ => return Err("Type invalide : ACHAT ou VENTE.".to_owned()),
    };
    let prix = check_price(input.prix, "Prix")?;
    if input.items.len() > ITEMS_MAX {
        return Err(format!("{ITEMS_MAX} objets au plus."));
    }
    let items = input.items.iter().map(check_item).collect::<Result<Vec<_>, _>>()?;
    Ok((groupe, kind, prix, items))
}

/// Statut choisi depuis le site : l'annulation passe par sa propre route, comme sur Discord.
pub fn check_status(raw: &str) -> Result<Status, String> {
    Status::parse(raw.trim()).filter(|s| Status::CHOICES.contains(s)).ok_or_else(|| "Statut inconnu.".to_owned())
}

async fn load(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> Result<Option<(Row, DbTime)>> {
    let gid = guild_id.to_string();
    let Some(row) = service::get(&ctx.db, &gid, id.trim()).await? else {
        return Ok(None);
    };
    let (created_at,): (DbTime,) = sqlx::query_as(r#"SELECT "createdAt" FROM "Transaction" WHERE "id" = ?"#)
        .bind(&row.id)
        .fetch_one(&ctx.db)
        .await?;
    Ok(Some((row, created_at)))
}

fn transaction(members: &Members, row: Row, created_at: DbTime) -> Transaction {
    let status = row.status();
    Transaction {
        id: row.id,
        created_at: iso_db(created_at),
        groupe: row.groupe,
        kind: row.kind,
        status: status.as_str().to_owned(),
        status_label: status.label(),
        prix: row.prix,
        creator: members.get(&row.created_by_id),
        note: row.note,
        items: row
            .items
            .into_iter()
            .map(|item| Item { nom: item.nom, quantite: item.quantite, prix_unitaire: item.prix_unitaire })
            .collect(),
    }
}

/// Transaction modifiable : existe sur ce serveur et n'est pas clôturée.
async fn open_transaction(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> ApiResult<Row> {
    let Some((row, _)) = load(ctx, guild_id, id).await? else {
        return Err(ApiError::not_found(NOT_FOUND));
    };
    if row.status() == Status::Cloturee {
        return Err(ApiError::invalid(CLOSED));
    }
    Ok(row)
}

/// Rafraîchit la fiche et le tableau de bord sans faire échouer l'action déjà faite.
async fn refresh(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) {
    if let Err(error) = service::refresh(ctx, &guild_id.to_string(), id).await {
        tracing::warn!(transaction = id, ?error, "fiche de la transaction non mise à jour");
    }
}

fn label(row: &Row) -> String {
    format!("**{}** — {} — {} $", row.groupe, type_label(&row.kind), format_price(row.prix))
}

async fn journal(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, action: Action, details: String) {
    let actor = session.actor_name(ctx, guild_id);
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, Entry::new(action, None, details)).await;
}

async fn respond(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> ApiResult<Transaction> {
    let (row, created_at) = load(ctx, guild_id, id).await?.ok_or_else(|| ApiError::not_found(NOT_FOUND))?;
    Ok(transaction(&Members::new(ctx, guild_id), row, created_at))
}

/// Crée une transaction et publie sa fiche dans le salon du tableau de bord (`transaction.utiliser`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/transactions", tag = "transactions", operation_id = "createTransaction",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = NewTransactionInput,
    responses(
        (status = 201, description = "Transaction créée", body = Transaction),
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
    JsonBody(input): JsonBody<NewTransactionInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionUtiliser)?;
    let gid = guild_id.to_string();
    let (groupe, kind, prix, items) = check_transaction(&input).map_err(ApiError::invalid)?;
    // Sur Discord, la fiche part dans le salon de la commande ; ici, dans celui du tableau de bord.
    let channel = service::get_config(&ctx.db, &gid).await?.and_then(|c| c.dashboard_channel_id);
    let Some(channel_id) = channel else {
        return Err(ApiError::invalid(
            "Aucun salon de tableau de bord des transactions : un admin doit lancer « /config transaction dashboard_channel:#salon », ou crée la transaction sur Discord.",
        ));
    };
    let id = service::create(
        &ctx.db,
        NewTransaction {
            guild_id: gid.clone(),
            channel_id,
            groupe,
            kind: kind.to_owned(),
            prix,
            created_by_id: session.viewer.id.to_string(),
            note: crate::modules::transactions::commands::clean_note(input.note.clone()),
        },
    )
    .await?;
    for item in &items {
        service::add_item(&ctx.db, &id, &item.nom, item.quantite, item.prix_unitaire).await?;
    }
    // C'est cette publication qui mentionne le rôle configuré, le cas échéant.
    refresh(&ctx, guild_id, &id).await;
    let response = respond(&ctx, guild_id, &id).await?;
    let details = format!(
        "**{}** — {} — {} $ · {} objet(s)",
        response.groupe,
        type_label(&response.kind),
        format_price(response.prix),
        items.len()
    );
    journal(&ctx, &session, guild_id, Action::TransactionCreate, details).await;
    Ok(created(&response))
}

/// Change le statut d'une transaction non clôturée (`transaction.utiliser`).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/transactions/{id}/status", tag = "transactions", operation_id = "setTransactionStatus",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la transaction")),
    request_body = TransactionStatusInput,
    responses(
        (status = 200, description = "Statut changé", body = Transaction),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée ou transaction clôturée", body = super::ErrorBody),
    )
)]
pub async fn set_status(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
    JsonBody(input): JsonBody<TransactionStatusInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionUtiliser)?;
    let status = check_status(&input.status).map_err(ApiError::invalid)?;
    let row = open_transaction(&ctx, guild_id, &id).await?;
    service::set_status(&ctx.db, &guild_id.to_string(), &row.id, status).await?;
    refresh(&ctx, guild_id, &row.id).await;
    let details = format!("{} : {} → {}", label(&row), row.status().label(), status.label());
    journal(&ctx, &session, guild_id, Action::TransactionStatus, details).await;
    Ok(json(&respond(&ctx, guild_id, &row.id).await?))
}

/// Ajoute un objet à une transaction non clôturée (`transaction.utiliser`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/transactions/{id}/items", tag = "transactions", operation_id = "addTransactionItem",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la transaction")),
    request_body = TransactionItemInput,
    responses(
        (status = 201, description = "Objet ajouté", body = Transaction),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée ou transaction clôturée", body = super::ErrorBody),
    )
)]
pub async fn add_item(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
    JsonBody(input): JsonBody<TransactionItemInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionUtiliser)?;
    let item = check_item(&input).map_err(ApiError::invalid)?;
    let row = open_transaction(&ctx, guild_id, &id).await?;
    if !service::add_item(&ctx.db, &row.id, &item.nom, item.quantite, item.prix_unitaire).await? {
        return Err(ApiError::not_found(NOT_FOUND));
    }
    refresh(&ctx, guild_id, &row.id).await;
    let details = format!("{} : {} × {}", label(&row), item.quantite, item.nom);
    journal(&ctx, &session, guild_id, Action::TransactionItem, details).await;
    Ok(created(&respond(&ctx, guild_id, &row.id).await?))
}

/// Annule une transaction non clôturée (`transaction.utiliser`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/transactions/{id}/cancel", tag = "transactions", operation_id = "cancelTransaction",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la transaction")),
    responses(
        (status = 200, description = "Transaction annulée", body = Transaction),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Transaction clôturée", body = super::ErrorBody),
    )
)]
pub async fn cancel(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionUtiliser)?;
    let row = open_transaction(&ctx, guild_id, &id).await?;
    service::set_status(&ctx.db, &guild_id.to_string(), &row.id, Status::Annulee).await?;
    refresh(&ctx, guild_id, &row.id).await;
    journal(&ctx, &session, guild_id, Action::TransactionCancel, label(&row)).await;
    Ok(json(&respond(&ctx, guild_id, &row.id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(nom: &str, quantite: Option<i64>, prix: Option<f64>) -> TransactionItemInput {
        TransactionItemInput { nom: nom.into(), quantite, prix_unitaire: prix }
    }

    #[test]
    fn validates_transactions() {
        let input = |groupe: &str, kind: &str, prix: f64, items: Vec<TransactionItemInput>| NewTransactionInput {
            groupe: groupe.into(),
            kind: kind.into(),
            prix,
            items,
            note: None,
        };
        let (groupe, kind, prix, items) =
            check_transaction(&input(" Ballas ", "ACHAT", 1500.5, vec![item(" Kit ", None, None)])).unwrap();
        assert_eq!((groupe.as_str(), kind, prix), ("Ballas", "ACHAT", 1500.5));
        assert_eq!(items, [CheckedItem { nom: "Kit".into(), quantite: 1, prix_unitaire: None }]);
        assert!(check_transaction(&input("  ", "ACHAT", 1.0, vec![])).unwrap_err().contains("groupe"));
        assert!(check_transaction(&input("x", "DON", 1.0, vec![])).unwrap_err().contains("Type"));
        assert!(check_transaction(&input("x", "VENTE", -1.0, vec![])).unwrap_err().contains("Prix"));
        assert!(check_transaction(&input("x", "VENTE", f64::NAN, vec![])).is_err());
        let many = (0..26).map(|_| item("x", None, None)).collect();
        assert!(check_transaction(&input("x", "VENTE", 1.0, many)).is_err());
        assert!(check_item(&item("x", Some(0), None)).unwrap_err().contains("Quantité"));
        assert!(check_item(&item("x", Some(2), Some(f64::INFINITY))).unwrap_err().contains("Prix unitaire"));
        assert!(check_item(&item("", Some(2), None)).is_err());
    }

    #[test]
    fn cancel_has_its_own_route() {
        assert_eq!(check_status("LIVRE_PAYE"), Ok(Status::LivrePaye));
        assert!(check_status("ANNULEE").is_err());
        assert!(check_status("PERDUE").is_err());
    }
}
