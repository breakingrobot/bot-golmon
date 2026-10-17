//! Bascule hebdomadaire (lundi 00:00, Europe/Paris) et filet quotidien de
//! rattrapage (00:05) au cas où le bot serait hors ligne le lundi à minuit.
//! `ensure_current_session` est idempotent : rejouer ne crée pas de doublon.
//! Le dimanche à partir de 18:00, rappel des avis manquants (une fois par session).

use super::{reminder, service};
use crate::{context::Context, util::time::TIMEZONE};
use chrono::{Datelike, Timelike, Utc, Weekday};
use std::time::Duration;

pub fn spawn(ctx: Context) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_run: Option<(i32, u32, u32, u32)> = None;
        loop {
            ticker.tick().await;
            let now = Utc::now();
            let local = now.with_timezone(&TIMEZONE);
            let weekly = local.weekday() == Weekday::Mon && local.hour() == 0 && local.minute() == 0;
            let daily = local.hour() == 0 && local.minute() == 5;
            let key = (local.year(), local.ordinal(), local.hour(), local.minute());
            if (weekly || daily) && last_run != Some(key) {
                last_run = Some(key);
                rollover(&ctx).await;
            }
            if reminder::reminder_due(now) {
                remind(&ctx).await;
            }
        }
    });
    tracing::info!("cron des recrues démarré (reset hebdomadaire le lundi, rattrapage quotidien, rappel le dimanche)");
}

async fn rollover(ctx: &Context) {
    let guild_ids: Vec<_> = ctx.cache.iter().guilds().map(|guild| guild.id()).collect();
    for guild_id in guild_ids {
        let result = async {
            service::ensure_current_session(ctx, guild_id).await?;
            service::sync_dashboard(ctx, guild_id).await
        }
        .await;
        if let Err(error) = result {
            tracing::error!(%guild_id, ?error, "bascule hebdomadaire des recrues échouée");
        }
    }
}

async fn remind(ctx: &Context) {
    let guild_ids: Vec<_> = ctx.cache.iter().guilds().map(|guild| guild.id()).collect();
    for guild_id in guild_ids {
        if let Err(error) = reminder::send_reminder(ctx, guild_id).await {
            tracing::error!(%guild_id, ?error, "rappel des avis recrues échoué");
        }
    }
}
