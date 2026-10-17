//! Tâches périodiques des présences (toutes les 5 minutes) : clôture, rappels
//! des non-répondants, rôle d'absence, présences planifiées et rapport
//! d'assiduité du lundi.

use super::{absences, service};
use crate::{
    context::Context,
    discord::messages::{self, Mentions, Outgoing, parse_id},
    util::{format::chunk_mentions, time::DbTime},
};
use chrono::Duration;
use std::time::Duration as StdDuration;

const INTERVAL: StdDuration = StdDuration::from_secs(5 * 60);
const REMINDER_INTERVAL_HOURS: i64 = 3;

pub fn spawn(ctx: Context) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Laisse au cache le temps de recevoir les guildes et leurs membres.
        tokio::time::sleep(StdDuration::from_secs(30)).await;
        loop {
            ticker.tick().await;
            closure_sweep(&ctx).await;
            reminder_sweep(&ctx).await;
            absence_sweep(&ctx).await;
            super::planning::sweep(&ctx).await;
            super::report::sweep(&ctx).await;
        }
    });
    tracing::info!("cron des présences démarré (contrôle toutes les 5 min, rappels espacés de 3 h)");
}

/// Clôture les présences dont l'heure est passée.
async fn closure_sweep(ctx: &Context) {
    let due: Vec<super::model::Presence> =
        match sqlx::query_as(r#"SELECT * FROM "Presence" WHERE "closedAt" IS NULL AND "eventDate" <= ?"#)
            .bind(DbTime::now())
            .fetch_all(&ctx.db)
            .await
        {
            Ok(rows) => rows,
            Err(error) => return tracing::error!(?error, "lecture des présences à clôturer"),
        };

    for presence in due {
        let result = async {
            // Message disparu ou jamais publié : la présence est supprimée, rien à clôturer.
            if !service::ensure_alive(ctx, &presence).await? {
                return Ok(());
            }
            service::close_presence(&ctx.db, &presence.id).await?;
            service::sync_message(ctx, &presence.id).await?;
            tracing::info!(presence = %presence.id, "présence clôturée automatiquement");
            anyhow::Ok(())
        }
        .await;
        if let Err(error) = result {
            tracing::error!(presence = %presence.id, ?error, "échec de la clôture de la présence");
        }
    }
}

/// Relance les non-répondants, au plus une fois toutes les 3 h par présence.
/// `lastReminderAt` est initialisé à la création (qui pingue déjà tout le monde).
async fn reminder_sweep(ctx: &Context) {
    let now = DbTime::now();
    let threshold = DbTime(now.0 - Duration::hours(REMINDER_INTERVAL_HOURS));
    let active: Vec<super::model::Presence> = match sqlx::query_as(
        r#"SELECT * FROM "Presence" WHERE "closedAt" IS NULL AND "eventDate" > ? AND ("lastReminderAt" IS NULL OR "lastReminderAt" <= ?)"#,
    )
    .bind(now)
    .bind(threshold)
    .fetch_all(&ctx.db)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return tracing::error!(?error, "lecture des présences à relancer"),
    };

    for presence in active {
        let result = async {
            // Pas de rappel pour une présence dont le message n'existe plus :
            // personne ne pourrait y répondre.
            if !service::ensure_alive(ctx, &presence).await? {
                return Ok(());
            }
            let ids = service::non_respondent_ids(ctx, &presence).await?;
            if ids.is_empty() {
                return Ok(());
            }
            let config = service::get_config(&ctx.db, &presence.guild_id).await?;
            let target = config.and_then(|c| c.reminder_channel_id).unwrap_or_else(|| presence.channel_id.clone());
            let Some(channel_id) = parse_id(&target) else {
                return Ok(());
            };

            let prefix = format!("⏰ Rappel — **{}** <t:{}:R>", presence.titre, presence.event_date.unix());
            for content in chunk_mentions(&ids, &prefix) {
                messages::send(ctx, channel_id, &Outgoing::text(content).with_mentions(Mentions::Users)).await?;
            }
            sqlx::query(r#"UPDATE "Presence" SET "lastReminderAt" = ? WHERE "id" = ?"#)
                .bind(now)
                .bind(&presence.id)
                .execute(&ctx.db)
                .await?;
            tracing::info!(presence = %presence.id, members = ids.len(), "rappel envoyé");
            anyhow::Ok(())
        }
        .await;
        if let Err(error) = result {
            tracing::error!(presence = %presence.id, ?error, "erreur lors du rappel");
        }
    }
}

/// Applique le rôle aux absences qui ont commencé et le retire à celles qui se
/// terminent (seulement si le membre n'a aucune autre absence active).
async fn absence_sweep(ctx: &Context) {
    match absences::started(&ctx.db).await {
        Ok(started) => {
            for absence in started {
                let Some(guild_id) = parse_id(&absence.guild_id) else { continue };
                // Rôle déjà porté : `apply_role` le voit dans le cache et n'appelle pas l'API.
                if let Err(error) = absences::apply_role(ctx, guild_id, &absence.user_id).await {
                    tracing::error!(absence = %absence.id, ?error, "application du rôle d'absence échouée");
                }
            }
        }
        Err(error) => tracing::error!(?error, "lecture des absences en cours"),
    }

    match absences::expired(&ctx.db).await {
        Ok(expired) => {
            for absence in expired {
                let Some(guild_id) = parse_id(&absence.guild_id) else { continue };
                match absences::end(ctx, guild_id, &absence.id, &absence.user_id).await {
                    Ok(_) => tracing::info!(absence = %absence.id, "absence terminée automatiquement"),
                    Err(error) => tracing::error!(absence = %absence.id, ?error, "clôture de l'absence échouée"),
                }
            }
        }
        Err(error) => tracing::error!(?error, "lecture des absences expirées"),
    }
}
