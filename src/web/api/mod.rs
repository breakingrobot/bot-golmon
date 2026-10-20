//! API JSON du site (`/api/*`), utilisée par l'interface React. Le contrat
//! (routes, paramètres, formes JSON, droits) est `ui/src/api/types.ts`.
//!
//! - Session : le même cookie signé que les pages ; les droits sont recalculés
//!   à chaque requête ([`Access`]).
//! - Erreurs : toujours du JSON `{ "error": "…" }` (+ `loginUrl` pour un 401),
//!   y compris pour une route inconnue.
//! - CSRF : toute écriture exige `Content-Type: application/json` et
//!   `X-Requested-With: fetch` (en plus du cookie `SameSite=Lax`).

mod absences;
mod annuaire;
mod coffres;
mod contacts;
mod cotisations;
mod journal;
mod live;
mod me;
mod members;
pub mod openapi;
mod overview;
mod permissions;
mod presences;
mod recrues;
mod reglages;
mod sante;
mod series;
mod stats;
pub(crate) mod tickets;
mod transaction_edits;
mod transactions;

#[cfg(test)]
mod tests;

use super::{Access, MaybeViewer, Viewer, auth};
use crate::{
    context::{Context, Ctx},
    discord::members as discord_members,
    modules::permissions::{Perm, denied},
    util::time::{DbTime, TIMEZONE},
};
use axum::{
    Router,
    body::Bytes,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::{HeaderMap, Method, StatusCode, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};

pub fn router() -> Router<Context> {
    Router::new()
        .route("/openapi.json", get(openapi::serve))
        .route("/me", get(me::me))
        .route("/guilds/{guild}/members", get(members::search))
        .route("/guilds/{guild}/members/{user}", get(members::profile))
        .route("/guilds/{guild}/stats", get(stats::stats))
        .route("/guilds/{guild}/transactions", get(transactions::list))
        .route("/guilds/{guild}/transactions/export.csv", get(transactions::export))
        .route("/guilds/{guild}/cotisations", get(cotisations::list).post(cotisations::create))
        .route("/guilds/{guild}/cotisations/export.csv", get(cotisations::export))
        .route("/guilds/{guild}/cotisations/objective", get(reglages::objective_get).put(reglages::objective_put))
        .route("/guilds/{guild}/cotisations/{id}", axum::routing::delete(cotisations::remove))
        .route("/guilds/{guild}/coffres/meta", get(coffres::meta))
        .route("/guilds/{guild}/coffres/stock", get(coffres::stock))
        .route("/guilds/{guild}/coffres/movements", get(coffres::movements))
        .route("/guilds/{guild}/coffres/players", get(coffres::players))
        .route("/guilds/{guild}/coffres/series", get(coffres::series))
        .route("/guilds/{guild}/coffres/export.csv", get(coffres::export))
        .route("/guilds/{guild}/coffres/alerts", get(reglages::alerts_get).put(reglages::alerts_put))
        .route("/guilds/{guild}/coffres/properties/{property}", put(coffres::name_property))
        .route("/guilds/{guild}/coffres/inventory", put(coffres::inventory))
        .route("/guilds/{guild}/coffres/players/{game_id}", put(coffres::link_player).delete(coffres::unlink_player))
        .route("/guilds/{guild}/contacts", get(contacts::list).post(contacts::create))
        .route("/guilds/{guild}/contacts/{id}", get(contacts::detail).patch(contacts::update).delete(contacts::remove))
        .route("/guilds/{guild}/contacts/{id}/notes", axum::routing::post(contacts::add_note))
        .route("/guilds/{guild}/journal", get(journal::journal))
        .route("/guilds/{guild}/permissions", get(permissions::show).put(permissions::update))
        .route("/guilds/{guild}/config", get(reglages::config_get))
        // Accueil, mise à jour en direct et santé.
        .route("/guilds/{guild}/overview", get(overview::overview))
        .route("/guilds/{guild}/events", get(live::events))
        .route("/guilds/{guild}/health", get(sante::health))
        // Présences, absences, recrues, annuaire, écritures des transactions.
        .route("/guilds/{guild}/presences", get(presences::list).post(presences::create))
        .route("/guilds/{guild}/presences/stats", get(presences::stats))
        .route("/guilds/{guild}/presences/export.csv", get(presences::export))
        .route("/guilds/{guild}/presences/plannings", get(presences::schedules).post(presences::create_schedule))
        .route("/guilds/{guild}/presences/plannings/{id}", axum::routing::delete(presences::delete_schedule))
        .route("/guilds/{guild}/presences/{id}", get(presences::detail))
        .route("/guilds/{guild}/presences/{id}/close", axum::routing::post(presences::close))
        .route("/guilds/{guild}/absences", get(absences::list).post(absences::create))
        .route("/guilds/{guild}/absences/{id}/end", axum::routing::post(absences::end))
        .route("/guilds/{guild}/recrues", get(recrues::list))
        .route("/guilds/{guild}/recrues/{id}/history", get(recrues::history))
        .route("/guilds/{guild}/recrues/{id}/vote", put(recrues::vote))
        .route("/guilds/{guild}/recrues/{id}/veto", put(recrues::set_veto).delete(recrues::clear_veto))
        .route("/guilds/{guild}/vehicules", get(annuaire::vehicles).post(annuaire::create_vehicle))
        .route("/guilds/{guild}/vehicules/{plate}", axum::routing::delete(annuaire::delete_vehicle))
        .route("/guilds/{guild}/numeros", get(annuaire::phones))
        .route("/guilds/{guild}/numeros/{user}", put(annuaire::set_phone).delete(annuaire::delete_phone))
        .route("/guilds/{guild}/transactions", axum::routing::post(transaction_edits::create))
        .route("/guilds/{guild}/transactions/{id}/status", put(transaction_edits::set_status))
        .route("/guilds/{guild}/transactions/{id}/items", axum::routing::post(transaction_edits::add_item))
        .route("/guilds/{guild}/transactions/{id}/cancel", axum::routing::post(transaction_edits::cancel))
        .route("/tickets", get(tickets::list))
        .route("/tickets/staff", get(tickets::staff))
        .route("/logs/{thread}", get(tickets::log))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware::from_fn(csrf))
}

// ---------------------------------------------------------------------------
// Erreurs

/// Erreur renvoyée en JSON (`ApiError` du contrat).
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    login_url: Option<String>,
}

/// Erreur : message lisible en français, lien de connexion pour un 401.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ApiError)]
struct ErrorBody {
    error: String,
    /// Seulement pour un 401 : lien de connexion.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = String)]
    login_url: Option<String>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into(), login_url: None }
    }

    /// 401 : pas connecté.
    pub fn unauthorized(ctx: &Ctx) -> Self {
        if !auth::enabled(ctx) {
            return Self::new(
                StatusCode::UNAUTHORIZED,
                "La connexion Discord n'est pas configurée (DISCORD_CLIENT_SECRET et WEB_URL).",
            );
        }
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "Connecte-toi avec Discord pour accéder à cette page.".to_owned(),
            login_url: Some(login_url()),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// 422 : saisie refusée (message affichable tel quel).
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    #[cfg(test)]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody { error: self.message, login_url: self.login_url };
        json_with(self.status, &body)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        tracing::error!(?error, "erreur de l'API");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne : réessaie dans un instant.")
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        anyhow::Error::from(error).into()
    }
}

pub type ApiResult<T = Response> = Result<T, ApiError>;

/// Lien de connexion qui ramène à l'accueil du site.
pub fn login_url() -> String {
    auth::login_href("/")
}

async fn not_found() -> ApiError {
    ApiError::not_found("Route inconnue.")
}

async fn method_not_allowed() -> ApiError {
    ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "Méthode non prise en charge sur cette route.")
}

/// Refuse d'enregistrer quoi que ce soit au nom d'une personne hors du groupe
/// (rôle membre, gradé ou lead), comme les commandes Discord.
pub async fn require_member(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> ApiResult<()> {
    if crate::modules::permissions::in_group(ctx, guild_id, user_id).await? {
        return Ok(());
    }
    let name = Members::new(ctx, guild_id).get(&user_id.to_string()).name;
    Err(ApiError::invalid(crate::modules::permissions::not_member(&name)))
}

// ---------------------------------------------------------------------------
// Réponses

fn json_with<T: Serialize + ?Sized>(status: StatusCode, value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => (
            status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8"), (header::CACHE_CONTROL, "no-store")],
            body,
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "sérialisation JSON impossible");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn json<T: Serialize + ?Sized>(value: &T) -> Response {
    json_with(StatusCode::OK, value)
}

pub fn created<T: Serialize + ?Sized>(value: &T) -> Response {
    json_with(StatusCode::CREATED, value)
}

pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Page d'une liste (`Page<T>` du contrat).
/// Schéma : un type concret par liste (`page_schema!` dans `openapi`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// Numéro de page demandé (1 par défaut, valeur invalide = 1).
pub fn page_number(raw: Option<&str>) -> i64 {
    raw.and_then(|p| p.trim().parse::<i64>().ok()).unwrap_or(1).max(1)
}

/// Page bornée au nombre de pages, et décalage SQL.
pub fn clamp_page(page: i64, total: i64, size: i64) -> (i64, i64) {
    let pages = ((total + size - 1) / size).max(1);
    let page = page.clamp(1, pages);
    (page, (page - 1) * size)
}

/// Instant en ISO 8601 UTC (`2026-10-18T17:20:13Z`).
pub fn iso(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn iso_db(time: DbTime) -> String {
    iso(time.0)
}

/// Jour de Paris (`YYYY-MM-DD`).
pub fn paris_day(time: DbTime) -> String {
    time.0.with_timezone(&TIMEZONE).format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------
// Membres

/// Membre Discord résolu par le bot (nom affiché sur le serveur).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberRef {
    pub id: String,
    pub name: String,
    #[schema(required = true)]
    pub avatar_url: Option<String>,
}

/// Noms et avatars des membres d'un serveur, lus dans le cache du bot (jamais
/// d'appel HTTP par ligne). Inconnu : l'identifiant tient lieu de nom.
#[derive(Clone, Copy)]
pub struct Members<'a> {
    pub ctx: &'a Ctx,
    pub guild_id: Id<GuildMarker>,
}

impl<'a> Members<'a> {
    pub fn new(ctx: &'a Ctx, guild_id: Id<GuildMarker>) -> Self {
        Self { ctx, guild_id }
    }

    pub fn get(&self, user_id: &str) -> MemberRef {
        self.get_or(user_id, None)
    }

    /// Comme [`get`](Self::get), avec un nom de repli (nom enregistré en base).
    pub fn get_or(&self, user_id: &str, fallback: Option<&str>) -> MemberRef {
        let id: Option<Id<UserMarker>> = user_id.trim().parse().ok().and_then(Id::new_checked);
        let name = id
            .and_then(|id| discord_members::display_name(self.ctx, self.guild_id, id))
            .or_else(|| fallback.filter(|f| !f.is_empty()).map(str::to_owned))
            .unwrap_or_else(|| user_id.to_owned());
        let avatar_url = id.and_then(|id| self.ctx.cache.user(id)).map(|user| discord_members::avatar_url(&user));
        MemberRef { id: user_id.to_owned(), name, avatar_url }
    }
}

/// Rôle Discord (`Role` du contrat).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = Role)]
pub struct RoleRef {
    pub id: String,
    pub name: String,
    #[schema(required = true)]
    pub color: Option<u32>,
}

/// Couleur principale d'un rôle (`None` : pas de couleur).
#[allow(deprecated)] // `colors` (dégradés) n'est pas exposé : la couleur principale suffit.
pub fn role_color(role: &twilight_model::guild::Role) -> Option<u32> {
    Some(role.color).filter(|c| *c != 0)
}

/// Rôle lu dans le cache ; inconnu : l'identifiant tient lieu de nom.
pub fn role_ref(ctx: &Ctx, role_id: &str) -> RoleRef {
    let role = role_id.parse().ok().and_then(Id::new_checked).and_then(|id| ctx.cache.role(id));
    match role {
        Some(role) => {
            RoleRef { id: role_id.to_owned(), name: role.resource().name.clone(), color: role_color(role.resource()) }
        }
        None => RoleRef { id: role_id.to_owned(), name: role_id.to_owned(), color: None },
    }
}

// ---------------------------------------------------------------------------
// Session et droits

/// Personne connectée et ses droits. Refus : 401 JSON.
pub struct Session {
    pub viewer: Viewer,
    pub access: Access,
}

impl FromRequestParts<Context> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, ctx: &Context) -> Result<Self, Self::Rejection> {
        let MaybeViewer(viewer) = match MaybeViewer::from_request_parts(parts, ctx).await {
            Ok(viewer) => viewer,
            Err(never) => match never {},
        };
        if !auth::enabled(ctx) {
            return Err(ApiError::unauthorized(ctx));
        }
        let Some(viewer) = viewer else {
            return Err(ApiError::unauthorized(ctx));
        };
        let access = Access::of(ctx, viewer.id).await;
        Ok(Self { viewer, access })
    }
}

/// Identifiant de serveur de l'adresse ; invalide : 404.
pub fn parse_guild(raw: &str) -> ApiResult<Id<GuildMarker>> {
    raw.trim().parse::<u64>().ok().and_then(Id::new_checked).ok_or_else(|| ApiError::not_found("Serveur introuvable."))
}

/// Identifiant de membre (snowflake) ; invalide : 422.
pub fn parse_user(raw: &str) -> ApiResult<Id<UserMarker>> {
    raw.trim().parse::<u64>().ok().and_then(Id::new_checked).ok_or_else(|| ApiError::invalid("Membre invalide."))
}

/// Contrôle d'accès à un serveur, sans connexion à Discord : testable seul.
pub fn check_guild(access: &Access, guild_id: Id<GuildMarker>, perm: Option<Perm>) -> ApiResult<()> {
    let Some(guild) = access.guild(guild_id) else {
        return Err(ApiError::forbidden("Tu n'as pas accès à ce serveur (il faut en être membre)."));
    };
    match perm {
        Some(perm) if !guild.can(perm) => Err(ApiError::forbidden(denied(perm, guild.level(perm)))),
        _ => Ok(()),
    }
}

/// Réservé à « Gérer le serveur ».
pub fn check_admin(access: &Access, guild_id: Id<GuildMarker>) -> ApiResult<()> {
    check_guild(access, guild_id, None)?;
    if access.is_admin(guild_id) { Ok(()) } else { Err(ApiError::forbidden("Réservé à « Gérer le serveur ».")) }
}

impl Session {
    /// Serveur de l'adresse, dont la personne est membre.
    pub fn member_of(&self, raw: &str) -> ApiResult<Id<GuildMarker>> {
        let guild_id = parse_guild(raw)?;
        check_guild(&self.access, guild_id, None)?;
        Ok(guild_id)
    }

    /// Serveur de l'adresse, où la personne a `perm`.
    pub fn require(&self, raw: &str, perm: Perm) -> ApiResult<Id<GuildMarker>> {
        let guild_id = parse_guild(raw)?;
        check_guild(&self.access, guild_id, Some(perm))?;
        Ok(guild_id)
    }

    pub fn require_admin(&self, raw: &str) -> ApiResult<Id<GuildMarker>> {
        let guild_id = parse_guild(raw)?;
        check_admin(&self.access, guild_id)?;
        Ok(guild_id)
    }

    pub fn can(&self, guild_id: Id<GuildMarker>, perm: Perm) -> bool {
        self.access.can(guild_id, perm)
    }

    /// Refus d'une action, avec le niveau requis.
    pub fn denied(&self, guild_id: Id<GuildMarker>, perm: Perm) -> ApiError {
        let level = self.access.guild(guild_id).map_or(perm.default_level(), |g| g.level(perm));
        ApiError::forbidden(denied(perm, level))
    }

    /// Nom de l'auteur pour le journal : pseudo du serveur, sinon nom Discord.
    pub fn actor_name(&self, ctx: &Ctx, guild_id: Id<GuildMarker>) -> String {
        discord_members::display_name(ctx, guild_id, self.viewer.id).unwrap_or_else(|| self.viewer.name.clone())
    }
}

// ---------------------------------------------------------------------------
// Extracteurs

/// Paramètres d'adresse ; invalides : 400 JSON.
pub struct ApiQuery<T>(pub T);

impl<T: DeserializeOwned + Send> FromRequestParts<Context> for ApiQuery<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _ctx: &Context) -> Result<Self, Self::Rejection> {
        Query::<T>::try_from_uri(&parts.uri)
            .map(|Query(value)| Self(value))
            .map_err(|error| ApiError::bad_request(format!("Paramètres invalides : {}", error.body_text())))
    }
}

/// Corps JSON ; invalide : 400 JSON.
pub struct JsonBody<T>(pub T);

impl<T: DeserializeOwned> FromRequest<Context> for JsonBody<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, ctx: &Context) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(request, ctx)
            .await
            .map_err(|_| ApiError::bad_request("Corps de la requête illisible."))?;
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|error| ApiError::bad_request(format!("Saisie invalide : {error}.")))
    }
}

/// Champ JSON qui distingue « absent » (`None`) de `null` (`Some(None)`).
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

// ---------------------------------------------------------------------------
// CSRF

const CSRF_REFUSED: &str =
    "Requête refusée : une écriture doit être envoyée en JSON avec l'en-tête « X-Requested-With: fetch ».";

/// Une écriture venue d'un autre site ne peut pas poser ces deux en-têtes sans
/// requête préalable CORS (refusée : aucun en-tête CORS n'est servi).
pub fn csrf_allowed(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    let fetch = headers
        .get("x-requested-with")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("fetch"));
    json && fetch
}

async fn csrf(request: Request, next: Next) -> Response {
    if !csrf_allowed(request.method(), request.headers()) {
        return ApiError::forbidden(CSRF_REFUSED).into_response();
    }
    next.run(request).await
}
