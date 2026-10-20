//! Page « Santé » (« Gérer le serveur ») : salons de coffre suivis et leur
//! webhook, connexion à Discord, processus, base, version et derniers
//! avertissements ou erreurs du journal.

use super::{Access, ApiQuery, ApiResult, Session, iso, json};
use crate::{
    context::{Context, Ctx},
    discord::messages::parse_id,
    modules::coffres::service as coffres,
    util::time::DbTime,
};
use axum::extract::{Path, State};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, GuildMarker},
};
use utoipa::{IntoParams, ToSchema};

/// Sans mouvement depuis ce nombre de jours, un salon suivi est signalé silencieux.
pub const SILENT_DAYS: i64 = 7;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HealthQuery {
    /// Jours sans mouvement avant l'alerte « webhook silencieux » (1 à 365, 7 par défaut).
    jours: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = HealthChannel)]
pub struct ChannelHealth {
    pub channel_id: String,
    /// Nom du salon (cache du bot), `null` s'il est introuvable.
    #[schema(required = true)]
    pub name: Option<String>,
    /// Le bot voit encore le salon.
    pub accessible: bool,
    /// Webhooks du jeu reconnus dans ce salon.
    pub webhooks: usize,
    #[schema(required = true)]
    pub last_movement_at: Option<String>,
    /// Dernier rattrapage depuis le démarrage (`null` : pas encore fait).
    #[schema(required = true)]
    pub last_catch_up_at: Option<String>,
    #[schema(required = true)]
    pub last_catch_up_ok: Option<bool>,
    /// Aucun mouvement depuis `silentDays` jours (ou jamais).
    pub silent: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = HealthShard)]
pub struct ShardInfo {
    pub id: u32,
    pub connected: bool,
    #[schema(required = true)]
    pub latency_ms: Option<u64>,
    /// Dernier événement reçu.
    pub seen_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = HealthLogEvent)]
pub struct LogInfo {
    pub at: String,
    #[schema(value_type = super::openapi::HealthLevel)]
    pub level: &'static str,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuildHealth {
    pub version: String,
    /// Commit compilé (`GIT_COMMIT` à la compilation), `null` s'il n'est pas connu.
    #[schema(required = true)]
    pub commit: Option<String>,
    pub started_at: String,
    pub uptime_seconds: u64,
    /// Taille de la base principale (journal WAL compris), `null` si illisible.
    #[schema(required = true)]
    pub database_bytes: Option<u64>,
    /// *Message Content Intent* actif (lecture des encadrés du jeu).
    pub message_content: bool,
    pub shards: Vec<ShardInfo>,
    pub silent_days: i64,
    pub channels: Vec<ChannelHealth>,
    /// Vue globale : erreurs de tous les serveurs et sans serveur (administrateur partout).
    pub global: bool,
    /// Derniers avertissements et erreurs, du plus récent au plus ancien.
    pub errors: Vec<LogInfo>,
}

/// Salon silencieux : aucun mouvement, ou le dernier date de plus de `days` jours.
pub fn is_silent(last: Option<DateTime<Utc>>, now: DateTime<Utc>, days: i64) -> bool {
    last.is_none_or(|last| now - last > Duration::days(days))
}

/// Vue globale : « Gérer le serveur » sur chacun des serveurs du bot.
pub fn global_view(bot_guilds: &[Id<GuildMarker>], access: &Access) -> bool {
    !bot_guilds.is_empty() && bot_guilds.iter().all(|id| access.is_admin(*id))
}

fn silent_days(raw: Option<&str>) -> i64 {
    raw.and_then(|d| d.trim().parse::<i64>().ok()).filter(|d| (1..=365).contains(d)).unwrap_or(SILENT_DAYS)
}

async fn database_bytes(ctx: &Ctx) -> Option<u64> {
    let path = &ctx.config.database_path;
    let main = tokio::fs::metadata(path).await.ok()?.len();
    let mut wal = path.clone().into_os_string();
    wal.push("-wal");
    let wal = tokio::fs::metadata(&wal).await.map_or(0, |m| m.len());
    Some(main + wal)
}

async fn channels(ctx: &Ctx, guild: &str, days: i64) -> anyhow::Result<Vec<ChannelHealth>> {
    let last: HashMap<String, DbTime> = sqlx::query_as::<_, (String, DbTime)>(
        r#"SELECT "channelId", MAX("at") FROM "CoffreMovement" WHERE "guildId" = ? GROUP BY "channelId""#,
    )
    .bind(guild)
    .fetch_all(&ctx.db)
    .await?
    .into_iter()
    .collect();
    let webhooks = coffres::webhooks(&ctx.db, guild, None).await?;
    let now = Utc::now();
    Ok(coffres::channels(&ctx.db, guild)
        .await?
        .into_iter()
        .map(|channel| {
            let cached = parse_id::<ChannelMarker>(&channel.channel_id).and_then(|id| ctx.cache.channel(id));
            let last_movement = last.get(&channel.channel_id).map(|at| at.0);
            let catch_up = ctx.health.catch_up(&channel.channel_id);
            ChannelHealth {
                name: cached.as_ref().and_then(|c| c.name.clone()),
                accessible: cached.is_some(),
                webhooks: webhooks.iter().filter(|w| w.channel_id == channel.channel_id).count(),
                last_movement_at: last_movement.map(iso),
                last_catch_up_at: catch_up.as_ref().map(|c| iso(c.at)),
                last_catch_up_ok: catch_up.map(|c| c.ok),
                silent: is_silent(last_movement, now, days),
                channel_id: channel.channel_id,
            }
        })
        .collect())
}

/// Santé du bot vue depuis un serveur (« Gérer le serveur »).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/health", tag = "reglages", operation_id = "getGuildHealth",
    params(("guild" = String, Path, description = "Identifiant du serveur"), HealthQuery),
    responses(
        (status = 200, description = "Santé", body = GuildHealth),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Réservé à « Gérer le serveur »", body = super::ErrorBody),
    )
)]
pub async fn health(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<HealthQuery>,
) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    let gid = guild_id.to_string();
    let days = silent_days(query.jours.as_deref());
    let bot_guilds: Vec<Id<GuildMarker>> =
        ctx.cache.iter().guilds().map(|g| g.id()).filter(|id| ctx.config.is_guild_allowed(*id)).collect();
    let global = global_view(&bot_guilds, &session.access);
    Ok(json(&GuildHealth {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: option_env!("GIT_COMMIT").filter(|c| !c.is_empty()).map(|c| c.chars().take(12).collect()),
        started_at: iso(ctx.health.started_at),
        uptime_seconds: ctx.health.uptime_secs(),
        database_bytes: database_bytes(&ctx).await,
        message_content: ctx.message_content,
        shards: ctx
            .health
            .shards()
            .into_iter()
            .map(|s| ShardInfo { id: s.id, connected: s.connected, latency_ms: s.latency_ms, seen_at: iso(s.seen_at) })
            .collect(),
        silent_days: days,
        channels: channels(&ctx, &gid, days).await?,
        global,
        errors: ctx
            .health
            .errors
            .for_guild(&gid, global)
            .into_iter()
            .map(|e| LogInfo { at: iso(e.at), level: e.level, target: e.target, message: e.message })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::permissions::{Grants, Holder, Roles},
        web::GuildAccess,
    };

    #[test]
    fn silent_after_the_chosen_number_of_days() {
        let now = Utc::now();
        assert!(is_silent(None, now, 7), "jamais de mouvement");
        assert!(!is_silent(Some(now - Duration::days(6)), now, 7));
        assert!(is_silent(Some(now - Duration::days(8)), now, 7));
        assert!(!is_silent(Some(now - Duration::days(8)), now, 10));
        assert_eq!((silent_days(None), silent_days(Some("3")), silent_days(Some("0"))), (7, 3, 7));
    }

    #[test]
    fn global_view_needs_admin_everywhere() {
        let guild = |id: u64, admin: bool| {
            let grants = Grants::from_parts(
                Roles::default(),
                HashMap::new(),
                Holder { roles: Vec::new(), manage_guild: admin, manage_messages: false },
            );
            GuildAccess::new(Id::new(id), id.to_string(), grants, admin)
        };
        let access = Access { tickets: false, guilds: vec![guild(1, true), guild(2, false)] };
        assert!(!global_view(&[Id::new(1), Id::new(2)], &access));
        assert!(global_view(&[Id::new(1)], &access));
        assert!(!global_view(&[Id::new(1), Id::new(3)], &access), "serveur du bot dont la personne n'est pas membre");
        assert!(!global_view(&[], &access));
    }
}
