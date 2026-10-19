//! Serveur web du bot, toujours démarré : `/healthz` pour la plateforme,
//! logs des tickets (si le module est configuré) et statistiques.
//!
//! Avec `DISCORD_CLIENT_SECRET` et une adresse publique, les pages sont
//! réservées aux personnes connectées avec Discord qui en ont le droit
//! ([`Access`]). Sans connexion configurée, seuls les logs restent lisibles,
//! par leur lien non devinable, comme dans modmailbot.

pub mod auth;
mod coffres;
mod contacts;
mod cotisations;
mod journal;
pub mod markdown;
pub mod pages;
mod stats;
mod transactions;

use crate::{
    context::{Context, Ctx},
    modules::permissions::{Grants, Holder, Perm},
};
use axum::{
    Router,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
    routing::get,
};
use twilight_model::{
    guild::Permissions,
    id::{
        Id,
        marker::{GuildMarker, RoleMarker, UserMarker},
    },
};

pub use auth::{MaybeViewer, Viewer};

pub const CSS: &str = include_str!("../../web/dist/app.css");
pub const JS: &str = include_str!("../../web/src/app.js");
pub const ASSET_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ce que la personne connectée peut consulter.
#[derive(Default)]
pub struct Access {
    /// Staff des tickets (`inboxServerPermission` sur le serveur inbox).
    pub tickets: bool,
    /// Serveurs dont elle est membre, avec ses droits (`/config permission`).
    pub guilds: Vec<GuildAccess>,
}

pub struct GuildAccess {
    pub id: Id<GuildMarker>,
    pub name: String,
    grants: Grants,
}

/// Pages d'un serveur, dans l'ordre d'affichage : permission, libellé, chemin
/// sous `/stats/<serveur>`.
const GUILD_PAGES: [(Perm, &str, &str); 6] = [
    (Perm::SiteStats, "Statistiques", ""),
    (Perm::TransactionHistorique, "Transactions", "/transactions"),
    (Perm::CotisationStats, "Cotisations", "/cotisations"),
    (Perm::CoffreVoir, "Coffres", "/coffres"),
    (Perm::ContactNotes, "Contacts", "/contacts"),
    (Perm::Journal, "Journal du staff", "/journal"),
];

impl Access {
    pub async fn of(ctx: &Ctx, user_id: Id<UserMarker>) -> Self {
        let mut access = Self::default();
        if let Some(modmail) = &ctx.modmail
            && let Some(roles) = member_roles(ctx, modmail.config.inbox_server_id, user_id).await
        {
            access.tickets = modmail.is_staff_member(ctx, user_id, &roles);
        }
        let guild_ids: Vec<Id<GuildMarker>> =
            ctx.cache.iter().guilds().map(|g| g.id()).filter(|id| ctx.config.is_guild_allowed(*id)).collect();
        for guild_id in guild_ids {
            let Some(roles) = member_roles(ctx, guild_id, user_id).await else {
                continue;
            };
            let permissions = crate::discord::members::guild_permissions(ctx, guild_id, user_id, &roles);
            let holder = Holder {
                roles: roles.iter().map(ToString::to_string).collect(),
                manage_guild: permissions.contains(Permissions::MANAGE_GUILD),
                manage_messages: permissions.contains(Permissions::MANAGE_MESSAGES),
            };
            let grants = match Grants::load(&ctx.db, guild_id, holder).await {
                Ok(grants) => grants,
                Err(error) => {
                    tracing::warn!(?error, %guild_id, "droits du site");
                    continue;
                }
            };
            let name = ctx.cache.guild(guild_id).map(|g| g.name().to_owned()).unwrap_or_else(|| guild_id.to_string());
            access.guilds.push(GuildAccess { id: guild_id, name, grants });
        }
        access.guilds.sort_by(|a, b| a.name.cmp(&b.name));
        access
    }

    pub fn can(&self, guild_id: Id<GuildMarker>, perm: Perm) -> bool {
        self.guilds.iter().any(|g| g.id == guild_id && g.grants.can(perm))
    }

    pub fn guild_name(&self, guild_id: Id<GuildMarker>) -> String {
        self.guilds.iter().find(|g| g.id == guild_id).map(|g| g.name.clone()).unwrap_or_else(|| guild_id.to_string())
    }

    /// Serveurs où la personne a `perm`.
    pub fn guilds_with(&self, perm: Perm) -> Vec<(Id<GuildMarker>, String)> {
        self.guilds.iter().filter(|g| g.grants.can(perm)).map(|g| (g.id, g.name.clone())).collect()
    }

    /// Pages du serveur ouvertes à la personne : (libellé, adresse).
    pub fn pages(&self, guild_id: Id<GuildMarker>) -> Vec<(&'static str, String)> {
        GUILD_PAGES
            .iter()
            .filter(|(perm, _, _)| self.can(guild_id, *perm))
            .map(|(_, label, path)| (*label, format!("/stats/{guild_id}{path}")))
            .collect()
    }
}

/// Rôles d'un membre : cache, sinon API. `None` si la personne n'est pas membre.
async fn member_roles(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Option<Vec<Id<RoleMarker>>> {
    if let Some(member) = ctx.cache.member(guild_id, user_id) {
        return Some(member.roles().to_vec());
    }
    let member = ctx.http.guild_member(guild_id, user_id).await.ok()?.model().await.ok()?;
    Some(member.roles)
}

/// Noms des mentions lus dans le cache du bot.
pub struct CacheResolver<'a>(pub &'a Ctx);

impl markdown::Resolver for CacheResolver<'_> {
    fn user(&self, id: u64) -> Option<String> {
        let user = self.0.cache.user(Id::new_checked(id)?)?;
        Some(user.global_name.clone().unwrap_or_else(|| user.name.clone()))
    }

    fn role(&self, id: u64) -> Option<String> {
        Some(self.0.cache.role(Id::new_checked(id)?)?.resource().name.clone())
    }

    fn channel(&self, id: u64) -> Option<String> {
        self.0.cache.channel(Id::new_checked(id)?)?.name.clone()
    }
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "public, max-age=86400")], body).into_response()
}

async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'self'; script-src 'self'; img-src * data:; base-uri 'none'; form-action 'self'",
        ),
    );
    response
}

/// Adresse d'écoute : celle du module ticket s'il est configuré, sinon `PORT` (8890 par défaut).
fn address(ctx: &Ctx) -> String {
    match &ctx.modmail {
        Some(modmail) => format!("{}:{}", modmail.config.host, modmail.config.port),
        None => format!("0.0.0.0:{}", ctx.config.web.port.unwrap_or(8890)),
    }
}

pub fn spawn(ctx: Context) {
    let address = address(&ctx);
    let mut app = Router::new()
        .route("/", get(pages::home))
        .route("/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/logout", get(auth::logout))
        .route("/stats", get(stats::index))
        .route("/stats/{guild_id}", get(stats::guild))
        .route("/stats/{guild_id}/journal", get(journal::page))
        .route("/cotisations", get(cotisations::index))
        .route("/stats/{guild_id}/cotisations", get(cotisations::page))
        .route("/stats/{guild_id}/cotisations.csv", get(cotisations::export))
        .route("/stats/{guild_id}/coffres", get(coffres::page))
        .route("/stats/{guild_id}/coffres.csv", get(coffres::export))
        .route("/contacts", get(contacts::index))
        .route("/stats/{guild_id}/contacts", get(contacts::page))
        .route("/stats/{guild_id}/transactions", get(transactions::history))
        .route("/stats/{guild_id}/transactions.csv", get(transactions::export))
        .route("/assets/app.css", get(|| async { asset("text/css; charset=UTF-8", CSS) }))
        .route("/assets/app.js", get(|| async { asset("text/javascript; charset=UTF-8", JS) }))
        .route("/healthz", get(|| async { "ok" }));
    if ctx.modmail.is_some() {
        app = app.merge(crate::modmail::web_routes());
    }
    let app = app.fallback(pages::not_found).layer(axum::middleware::map_response(security_headers)).with_state(ctx);

    tokio::spawn(async move {
        match tokio::net::TcpListener::bind(&address).await {
            Ok(listener) => {
                tracing::info!(%address, "serveur web démarré");
                if let Err(error) = axum::serve(listener, app).await {
                    tracing::warn!(%error, "erreur du serveur web");
                }
            }
            Err(error) => tracing::warn!(%address, %error, "serveur web impossible à démarrer"),
        }
    });
}
