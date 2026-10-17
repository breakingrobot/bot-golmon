//! Serveur web du bot, toujours démarré : `/healthz` pour la plateforme,
//! logs des tickets (si le module est configuré) et statistiques.
//!
//! Avec `DISCORD_CLIENT_SECRET` et une adresse publique, les pages sont
//! réservées aux personnes connectées avec Discord qui en ont le droit
//! ([`Access`]). Sans connexion configurée, seuls les logs restent lisibles,
//! par leur lien non devinable, comme dans modmailbot.

pub mod auth;
pub mod pages;
mod stats;

use crate::context::{Context, Ctx};
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
#[derive(Clone, Debug, Default)]
pub struct Access {
    /// Staff des tickets (`inboxServerPermission` sur le serveur inbox).
    pub tickets: bool,
    /// Serveurs dont elle peut voir les statistiques (« Gérer le serveur »).
    pub guilds: Vec<(Id<GuildMarker>, String)>,
}

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
            if member_permissions(ctx, guild_id, user_id, &roles).contains(Permissions::MANAGE_GUILD) {
                let name =
                    ctx.cache.guild(guild_id).map(|g| g.name().to_owned()).unwrap_or_else(|| guild_id.to_string());
                access.guilds.push((guild_id, name));
            }
        }
        access.guilds.sort_by(|a, b| a.1.cmp(&b.1));
        access
    }

    pub fn can_view_guild(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guilds.iter().any(|(id, _)| *id == guild_id)
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

fn member_permissions(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    roles: &[Id<RoleMarker>],
) -> Permissions {
    if ctx.cache.guild(guild_id).is_some_and(|g| g.owner_id() == user_id) {
        return Permissions::all();
    }
    let everyone = ctx.cache.role(guild_id.cast()).map(|r| r.resource().permissions).unwrap_or_else(Permissions::empty);
    let permissions =
        roles.iter().filter_map(|id| ctx.cache.role(*id)).fold(everyone, |acc, role| acc | role.resource().permissions);
    if permissions.contains(Permissions::ADMINISTRATOR) { Permissions::all() } else { permissions }
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
