//! Annuaire : véhicules du groupe (`vehicule.utiliser`, comme `/vehicle`) et
//! numéros des membres (membres du groupe, comme `/numero`). Les écritures
//! mettent à jour les tableaux de bord publiés sur Discord. Aucune lecture
//! n'interroge l'API GLife ; l'ajout d'un véhicule sans modèle la consulte une
//! fois, comme la commande, sans jamais bloquer l'ajout.

use super::{
    ApiError, ApiQuery, ApiResult, JsonBody, MemberRef, Members, RoleRef, Session, absences::check_group, created,
    json, no_content, permissions::guild_roles, role_ref,
};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        permissions::Perm,
        phones::{self, INVALID_FORMAT, service as phone_service},
        vehicles::{
            api::{Lookup, normalize_plate},
            service as vehicle_service,
        },
    },
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::{IntoParams, ToSchema};

const PLATE_MAX: usize = 12;
const MODEL_MAX: usize = 60;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleEntry {
    /// Plaque normalisée (majuscules, sans espace ni tiret).
    pub plate: String,
    #[schema(required = true)]
    pub model: Option<String>,
    /// Propriétaire membre (`null` si c'est un rôle).
    #[schema(required = true)]
    pub owner_member: Option<MemberRef>,
    /// Propriétaire rôle (`null` si c'est un membre).
    #[schema(required = true)]
    pub owner_role: Option<RoleRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehiclesResponse {
    pub items: Vec<VehicleEntry>,
    /// Rôles du serveur, pour choisir un rôle propriétaire (le plus haut d'abord).
    pub roles: Vec<RoleRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PhoneEntry {
    pub member: MemberRef,
    /// Format affiché : `555-1234`.
    pub number: String,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DirectoryQuery {
    /// Texte cherché (plaque, modèle, propriétaire ; nom ou numéro).
    q: Option<String>,
}

/// Recherche insensible à la casse et aux tirets : `d6cs-3p` trouve `D6CS3P0G`.
fn simplify(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| !c.is_whitespace() && *c != '-').collect()
}

pub fn matches(query: &str, fields: &[&str]) -> bool {
    let query = simplify(query);
    query.is_empty() || fields.iter().any(|field| simplify(field).contains(&query))
}

fn vehicle(ctx: &Ctx, members: &Members, row: &vehicle_service::Vehicle) -> VehicleEntry {
    VehicleEntry {
        plate: row.plate.clone(),
        model: row.modele.clone(),
        owner_member: (!row.owner_is_role).then(|| members.get(&row.owner_id)),
        owner_role: row.owner_is_role.then(|| role_ref(ctx, &row.owner_id)),
    }
}

/// Véhicules enregistrés (`vehicule.utiliser`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/vehicules", tag = "annuaire", operation_id = "getVehicles",
    params(("guild" = String, Path, description = "Identifiant du serveur"), DirectoryQuery),
    responses(
        (status = 200, description = "Véhicules", body = VehiclesResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn vehicles(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<DirectoryQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::Vehicule)?;
    let members = Members::new(&ctx, guild_id);
    let q = query.q.unwrap_or_default();
    let items = vehicle_service::list(&ctx.db, &guild_id.to_string())
        .await?
        .iter()
        .map(|row| vehicle(&ctx, &members, row))
        .filter(|v| {
            let owner =
                v.owner_member.as_ref().map(|m| m.name.as_str()).or(v.owner_role.as_ref().map(|r| r.name.as_str()));
            matches(&q, &[&v.plate, v.model.as_deref().unwrap_or_default(), owner.unwrap_or_default()])
        })
        .collect();
    Ok(json(&VehiclesResponse { items, roles: guild_roles(&ctx, guild_id) }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VehicleInput {
    pub plate: String,
    /// Identifiant Discord d'un membre (du groupe) ou d'un rôle.
    pub owner_id: String,
    #[serde(default)]
    pub owner_is_role: bool,
    /// Absent : modèle demandé à l'API GLife (un échec n'empêche pas l'ajout).
    #[serde(default)]
    pub model: Option<String>,
}

/// Plaque et modèle validés ; `Err` : message affichable.
pub fn check_vehicle(input: &VehicleInput) -> Result<(String, Option<String>), String> {
    let plate = normalize_plate(&input.plate);
    if plate.is_empty() || plate.chars().count() > PLATE_MAX || !plate.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Plaque invalide.".to_owned());
    }
    let model = input.model.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned);
    if model.as_ref().is_some_and(|m| m.chars().count() > MODEL_MAX) {
        return Err(format!("Le modèle dépasse {MODEL_MAX} caractères."));
    }
    Ok((plate, model))
}

fn already_registered(plate: &str) -> String {
    format!(
        "La plaque {plate} est déjà enregistrée. Retire-la ou change son propriétaire sur Discord (« /vehicle owner »)."
    )
}

/// Met à jour le tableau de bord sans faire échouer l'action déjà faite.
async fn refresh_vehicles(ctx: &Ctx, guild: &str) {
    if let Err(error) = vehicle_service::sync_dashboard(ctx, guild).await {
        tracing::warn!(?error, "tableau de bord des véhicules non mis à jour");
    }
}

/// Enregistre un véhicule (`vehicule.utiliser`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/vehicules", tag = "annuaire", operation_id = "createVehicle",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = VehicleInput,
    responses(
        (status = 201, description = "Véhicule enregistré", body = VehicleEntry),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn create_vehicle(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(input): JsonBody<VehicleInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::Vehicule)?;
    let gid = guild_id.to_string();
    let (plate, mut model) = check_vehicle(&input).map_err(ApiError::invalid)?;
    let owner_id = input.owner_id.trim().to_owned();
    if input.owner_is_role {
        let known = owner_id.parse().ok().and_then(Id::new_checked).and_then(|id| ctx.cache.role(id));
        if known.is_none_or(|role| role.guild_id() != guild_id) {
            return Err(ApiError::invalid("Rôle introuvable sur ce serveur."));
        }
    } else {
        // Un véhicule au nom d'un membre : seulement s'il fait partie du groupe.
        super::require_member(&ctx, guild_id, super::parse_user(&owner_id)?).await?;
    }
    if vehicle_service::get_by_plate(&ctx.db, &gid, &plate).await?.is_some() {
        return Err(ApiError::invalid(already_registered(&plate)));
    }
    // Pré-remplissage : une API indisponible ne doit jamais bloquer l'ajout.
    if model.is_none()
        && let Lookup::Found(found) = ctx.vehicles.lookup(&plate).await
    {
        model = found.name.or(found.model);
    }
    if !vehicle_service::add(&ctx.db, &gid, &plate, &owner_id, input.owner_is_role, model.as_deref()).await? {
        return Err(ApiError::invalid(already_registered(&plate)));
    }
    refresh_vehicles(&ctx, &gid).await;
    let row = vehicle_service::Vehicle { plate, owner_id, owner_is_role: input.owner_is_role, modele: model };
    Ok(created(&vehicle(&ctx, &Members::new(&ctx, guild_id), &row)))
}

/// Retire un véhicule du registre (`vehicule.utiliser`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/vehicules/{plate}", tag = "annuaire", operation_id = "deleteVehicle",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("plate" = String, Path, description = "Plaque")),
    responses(
        (status = 204, description = "Retiré"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn delete_vehicle(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, plate)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::Vehicule)?;
    let gid = guild_id.to_string();
    if vehicle_service::get_by_plate(&ctx.db, &gid, &plate).await?.is_none() {
        return Err(ApiError::not_found(format!(
            "Aucun véhicule enregistré avec la plaque {}.",
            normalize_plate(&plate)
        )));
    }
    vehicle_service::remove(&ctx.db, &gid, &plate).await?;
    refresh_vehicles(&ctx, &gid).await;
    Ok(no_content())
}

// ---------------------------------------------------------------------------
// Numéros

/// Numéros des membres (membres du groupe, comme `/numero voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/numeros", tag = "annuaire", operation_id = "getPhones",
    params(("guild" = String, Path, description = "Identifiant du serveur"), DirectoryQuery),
    responses(
        (status = 200, description = "Numéros", body = Vec<PhoneEntry>),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Pas membre du groupe", body = super::ErrorBody),
    )
)]
pub async fn phones(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<DirectoryQuery>,
) -> ApiResult {
    let guild_id = super::parse_guild(&guild)?;
    check_group(&session.access, guild_id)?;
    let members = Members::new(&ctx, guild_id);
    let q = query.q.unwrap_or_default();
    let mut items: Vec<PhoneEntry> = phone_service::list(&ctx.db, &guild_id.to_string())
        .await?
        .iter()
        .map(|row| PhoneEntry { member: members.get(&row.user_id), number: phones::format(&row.numero) })
        .filter(|p| matches(&q, &[&p.member.name, &p.number]))
        .collect();
    items.sort_by_key(|p| p.member.name.to_lowercase());
    Ok(json(&items))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PhoneInput {
    /// `555` suivi de 4 à 6 chiffres (`555-1234` accepté).
    pub number: String,
}

async fn refresh_phones(ctx: &Ctx, guild: &str) {
    if let Err(error) = phone_service::sync_dashboard(ctx, guild).await {
        tracing::warn!(?error, "annuaire des numéros non mis à jour");
    }
}

async fn journal(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, entry: Entry) {
    let actor = session.actor_name(ctx, guild_id);
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, entry).await;
}

/// Enregistre un numéro : le sien (membre du groupe), celui d'un autre (`numero.ajouter_autrui`).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/numeros/{user}", tag = "annuaire", operation_id = "setPhone",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("user" = String, Path, description = "Identifiant Discord du membre")),
    request_body = PhoneInput,
    responses(
        (status = 200, description = "Numéro enregistré", body = PhoneEntry),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn set_phone(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, user)): Path<(String, String)>,
    JsonBody(input): JsonBody<PhoneInput>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let target = super::parse_user(&user)?;
    let for_someone_else = target != session.viewer.id;
    if for_someone_else && !session.can(guild_id, Perm::NumeroAjouterAutrui) {
        return Err(session.denied(guild_id, Perm::NumeroAjouterAutrui));
    }
    let numero = phones::normalize(&input.number);
    if !phones::is_valid(&numero) {
        return Err(ApiError::invalid(INVALID_FORMAT.replace('`', "")));
    }
    super::require_member(&ctx, guild_id, target).await?;
    let gid = guild_id.to_string();
    if !phone_service::set(&ctx.db, &gid, &target.to_string(), &numero).await? {
        return Err(ApiError::invalid(phone_service::NUMBER_TAKEN));
    }
    refresh_phones(&ctx, &gid).await;
    if for_someone_else {
        journal(
            &ctx,
            &session,
            guild_id,
            Entry::new(Action::PhoneSetFor, Some(target.to_string()), phones::format(&numero)),
        )
        .await;
    }
    let members = Members::new(&ctx, guild_id);
    Ok(json(&PhoneEntry { member: members.get(&target.to_string()), number: phones::format(&numero) }))
}

/// Supprime un numéro : le sien, celui d'un autre (`numero.supprimer_autrui`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/numeros/{user}", tag = "annuaire", operation_id = "deletePhone",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("user" = String, Path, description = "Identifiant Discord du membre")),
    responses(
        (status = 204, description = "Supprimé"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Aucun numéro", body = super::ErrorBody),
    )
)]
pub async fn delete_phone(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, user)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let target = super::parse_user(&user)?;
    let for_someone_else = target != session.viewer.id;
    if for_someone_else && !session.can(guild_id, Perm::NumeroSupprimerAutrui) {
        return Err(session.denied(guild_id, Perm::NumeroSupprimerAutrui));
    }
    let gid = guild_id.to_string();
    if !phone_service::remove(&ctx.db, &gid, &target.to_string()).await? {
        return Err(ApiError::not_found("Aucun numéro enregistré pour ce membre."));
    }
    refresh_phones(&ctx, &gid).await;
    if for_someone_else {
        journal(&ctx, &session, guild_id, Entry::new(Action::PhoneRemovedFor, Some(target.to_string()), String::new()))
            .await;
    }
    Ok(no_content())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ignores_case_and_dashes() {
        assert!(matches("d6cs-3p", &["D6CS3P0G", "Sultan"]));
        assert!(matches("sul", &["D6CS3P0G", "Sultan"]));
        assert!(matches("555 01", &["Elodie", "555-0101"]));
        assert!(!matches("zz", &["D6CS3P0G", "Sultan"]));
        assert!(matches("  ", &[]));
    }

    #[test]
    fn checks_plates_and_models() {
        let input = |plate: &str, model: Option<&str>| VehicleInput {
            plate: plate.into(),
            owner_id: "1".into(),
            owner_is_role: false,
            model: model.map(Into::into),
        };
        assert_eq!(check_vehicle(&input(" d6cs 3p0g ", Some("  "))), Ok(("D6CS3P0G".to_owned(), None)));
        assert_eq!(check_vehicle(&input("AB-12", Some(" Sultan "))).unwrap().1.as_deref(), Some("Sultan"));
        assert!(check_vehicle(&input("  ", None)).is_err());
        assert!(check_vehicle(&input("AB;12", None)).is_err());
        assert!(check_vehicle(&input("A".repeat(13).as_str(), None)).is_err());
        assert!(check_vehicle(&input("AB12", Some(&"x".repeat(61)))).is_err());
    }
}
