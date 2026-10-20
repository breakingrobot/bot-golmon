//! Passage des rappels toutes les 5 minutes : présences sans réponse qui
//! approchent, puis, le samedi soir, objectif de cotisation.

use super::{Config, Kind, objective_due, presence_due, presence_text, recipients, service};
use crate::{
    context::{Context, Ctx},
    discord::messages::{self, Outgoing, parse_id},
    modules::{
        cotisations::objective,
        presences::{self, model::Presence},
    },
    util::time::{DbTime, week_start},
};
use anyhow::Result;
use chrono::{Duration, Utc};
use std::{collections::HashSet, time::Duration as StdDuration};
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_model::id::{Id, marker::GuildMarker};

const INTERVAL: StdDuration = StdDuration::from_secs(5 * 60);

pub fn spawn(ctx: Context) {
    tokio::spawn(async move {
        // Laisse au cache le temps de recevoir les guildes et leurs membres.
        tokio::time::sleep(StdDuration::from_secs(45)).await;
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            sweep(&ctx).await;
        }
    });
    tracing::info!("cron des rappels en MP démarré (contrôle toutes les 5 min)");
}

async fn sweep(ctx: &Ctx) {
    let guilds = match service::enabled_guilds(&ctx.db).await {
        Ok(guilds) => guilds,
        Err(error) => return tracing::error!(?error, "lecture des serveurs aux rappels activés"),
    };
    for (guild, config) in guilds {
        let Some(guild_id) = parse_id::<GuildMarker>(&guild) else { continue };
        // Cache des membres pas encore rempli (redémarrage) : au passage suivant.
        if ctx.cache.guild_members(guild_id).is_none_or(|members| members.is_empty()) {
            continue;
        }
        if let Err(error) = presence_reminders(ctx, guild_id, &config).await {
            tracing::error!(%guild_id, ?error, "rappels de présence en MP échoués");
        }
        if config.objective
            && objective_due(Utc::now())
            && let Err(error) = objective_reminders(ctx, guild_id).await
        {
            tracing::error!(%guild_id, ?error, "rappels de l'objectif en MP échoués");
        }
    }
    if let Err(error) = service::purge_sent(&ctx.db).await {
        tracing::warn!(?error, "purge des rappels envoyés");
    }
}

/// Bots, exclus des rappels.
fn bots(ctx: &Ctx, ids: &[String]) -> HashSet<String> {
    ids.iter()
        .filter(|id| parse_id(id).and_then(|id| ctx.cache.user(id)).is_some_and(|user| user.bot))
        .cloned()
        .collect()
}

fn message_link(presence: &Presence) -> Option<String> {
    let message = presence.message_id.as_deref()?;
    Some(format!("https://discord.com/channels/{}/{}/{message}", presence.guild_id, presence.channel_id))
}

async fn presence_reminders(ctx: &Ctx, guild_id: Id<GuildMarker>, config: &Config) -> Result<()> {
    let guild = guild_id.to_string();
    let now = Utc::now();
    let until = DbTime(now + Duration::hours(config.presence_hours));
    let presences = service::upcoming_presences(&ctx.db, &guild, until).await?;
    if presences.is_empty() {
        return Ok(());
    }
    let absence_role = presences::service::get_config(&ctx.db, &guild).await?.and_then(|c| c.absence_role_id);
    let prefs = service::prefs(&ctx.db, &guild).await?;
    let guild_name = ctx.cache.guild(guild_id).map(|g| g.name().to_owned());
    for presence in presences {
        if !presence_due(now, presence.event_date, presence.created_at, config.presence_hours) {
            continue;
        }
        // Mêmes attendus que les relances : sans réponse, hors absents d'office et blocklist.
        let pending = presences::service::non_respondent_ids(ctx, &presence).await?;
        let mut excluded: HashSet<String> =
            presences::service::auto_absent_user_ids(ctx, guild_id, presence.event_date, absence_role.as_deref())
                .await?
                .into_iter()
                .collect();
        excluded.extend(bots(ctx, &pending));
        let text = presence_text(
            guild_name.as_deref(),
            &presence.titre,
            presence.event_date,
            message_link(&presence).as_deref(),
        );
        for user in recipients(&pending, &excluded, &prefs, Kind::Presence, now) {
            if service::claim(&ctx.db, &guild, Kind::Presence, &presence.id, &user).await? {
                send_dm(ctx, &guild, &user, &text).await?;
            }
        }
    }
    Ok(())
}

async fn objective_reminders(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(goal) = objective::get_objective(&ctx.db, &guild).await? else {
        return Ok(());
    };
    let now = Utc::now();
    let week = week_start(now);
    let tracked = objective::tracked_members(ctx, guild_id).await?;
    let progress = objective::progress_for(&ctx.db, &guild, week, &goal, &tracked).await?;
    let below: Vec<String> = progress.iter().filter(|r| !r.exempt && !r.reached).map(|r| r.user_id.clone()).collect();
    let prefs = service::prefs(&ctx.db, &guild).await?;
    let guild_name = ctx.cache.guild(guild_id).map(|g| g.name().to_owned());
    let reference = week.to_db_string();
    for user in recipients(&below, &HashSet::new(), &prefs, Kind::Objective, now) {
        let Some(row) = progress.iter().find(|r| r.user_id == user) else { continue };
        if service::claim(&ctx.db, &guild, Kind::Objective, &reference, &user).await? {
            send_dm(ctx, &guild, &user, &super::objective_text(guild_name.as_deref(), &goal, row)).await?;
        }
    }
    Ok(())
}

/// Erreur Discord 50007 : messages privés fermés (ou bot bloqué).
fn dm_closed(error: &anyhow::Error) -> bool {
    error.downcast_ref::<twilight_http::Error>().is_some_and(|error| {
        matches!(error.kind(), ErrorType::Response { error: ApiError::General(general), .. } if general.code == 50007)
    })
}

/// Envoie un MP. Refusé : noté, sans erreur ; le rappel n'est pas retenté.
async fn send_dm(ctx: &Ctx, guild: &str, user: &str, text: &str) -> Result<()> {
    let Some(user_id) = parse_id(user) else {
        return Ok(());
    };
    let result = async {
        let channel = ctx.http.create_private_channel(user_id).await?.model().await?;
        messages::send(ctx, channel.id, &Outgoing::text(text)).await
    }
    .await;
    match result {
        Ok(_) => Ok(()),
        Err(error) if dm_closed(&error) => service::mark_dm_closed(&ctx.db, guild, user).await,
        Err(error) => {
            tracing::warn!(%user, ?error, "rappel en MP non envoyé");
            Ok(())
        }
    }
}
