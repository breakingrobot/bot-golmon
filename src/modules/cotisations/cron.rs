//! Tâches de l'objectif hebdomadaire, vérifiées chaque minute :
//! - le dimanche à partir de 18:00 (Paris), rappel aux membres sous l'objectif
//!   (mentionnés), une fois par semaine ;
//! - le lundi à partir de 09:00, bilan de la semaine écoulée (sans notification) ;
//! - au changement de semaine, mise à jour du tableau de bord.
//!
//! Les messages partent dans le salon des annonces, sinon celui du tableau.

use super::{
    dashboard, money,
    objective::{self, Objective, ProgressRow},
};
use crate::{
    context::{Context, Ctx},
    discord::{
        components::{container, separator, text_display, text_displays},
        messages::{self, Mentions, Outgoing, parse_id},
    },
    util::{
        format::{ListBlock, V2_TEXT_LIMIT, pack_blocks},
        time::{DbTime, TIMEZONE, week_start},
    },
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use sqlx::SqlitePool;
use std::time::Duration;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::GuildMarker},
};

pub const REMINDER_HOUR: u32 = 18;
pub const RECAP_HOUR: u32 = 9;
/// Discord refuse plus de 100 utilisateurs dans `allowed_mentions` : marge confortable.
const MENTIONS_PER_MESSAGE: usize = 50;
const MAX_MESSAGE_LENGTH: usize = 1900;

pub fn spawn(ctx: Context) {
    tokio::spawn(async move {
        // Laisse au cache le temps de recevoir les guildes et leurs membres.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            sweep(&ctx, Utc::now()).await;
        }
    });
    tracing::info!("cron des cotisations démarré (rappel le dimanche, bilan le lundi, tableau hebdomadaire)");
}

/// Dimanche à partir de 18:00, heure de Paris.
pub fn reminder_due(now: DateTime<Utc>) -> bool {
    let local = now.with_timezone(&TIMEZONE);
    local.weekday() == Weekday::Sun && local.hour() >= REMINDER_HOUR
}

/// Lundi à partir de 09:00, heure de Paris.
pub fn recap_due(now: DateTime<Utc>) -> bool {
    let local = now.with_timezone(&TIMEZONE);
    local.weekday() == Weekday::Mon && local.hour() >= RECAP_HOUR
}

/// Cache des membres rempli (sinon les membres suivis seraient introuvables).
fn members_ready(ctx: &Ctx, guild_id: Id<GuildMarker>) -> bool {
    ctx.cache.guild_members(guild_id).is_some_and(|members| !members.is_empty())
}

async fn sweep(ctx: &Ctx, now: DateTime<Utc>) {
    let week = week_start(now);
    if reminder_due(now) {
        match pending(&ctx.db, "lastReminderAt", week).await {
            Ok(guilds) => {
                for guild_id in guilds.iter().filter_map(|g| parse_id(g)) {
                    if let Err(error) = send_reminder(ctx, guild_id, week).await {
                        tracing::error!(%guild_id, ?error, "rappel de l'objectif de cotisation échoué");
                    }
                }
            }
            Err(error) => tracing::error!(?error, "lecture des rappels de cotisation"),
        }
    }
    if recap_due(now) {
        match pending(&ctx.db, "lastRecapAt", week).await {
            Ok(guilds) => {
                for guild_id in guilds.iter().filter_map(|g| parse_id(g)) {
                    if let Err(error) = send_recap(ctx, guild_id, week).await {
                        tracing::error!(%guild_id, ?error, "bilan de l'objectif de cotisation échoué");
                    }
                }
            }
            Err(error) => tracing::error!(?error, "lecture des bilans de cotisation"),
        }
    }
    match dashboard::stale_dashboards(&ctx.db, week).await {
        Ok(guilds) => {
            for guild_id in guilds.iter().filter_map(|g| parse_id(g)) {
                if members_ready(ctx, guild_id) {
                    dashboard::refresh(ctx, guild_id).await;
                }
            }
        }
        Err(error) => tracing::error!(?error, "lecture des tableaux de cotisation"),
    }
}

/// Serveurs avec un objectif dont l'envoi (`column`) n'a pas eu lieu cette semaine.
async fn pending(db: &SqlitePool, column: &str, week: DbTime) -> Result<Vec<String>> {
    let reminder = if column == "lastReminderAt" { r#" AND "objectiveReminder" = 1"# } else { "" };
    let rows: Vec<(String,)> = sqlx::query_as(&format!(
        r#"SELECT "guildId" FROM "CotisationConfig"
           WHERE "objectiveAmount" > 0{reminder} AND ("{column}" IS NULL OR "{column}" < ?)"#
    ))
    .bind(week)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Réserve l'envoi de la semaine : `true` pour un seul appelant.
async fn claim(db: &SqlitePool, column: &str, guild_id: &str, week: DbTime) -> Result<bool> {
    let result = sqlx::query(&format!(
        r#"UPDATE "CotisationConfig" SET "{column}" = ? WHERE "guildId" = ? AND ("{column}" IS NULL OR "{column}" < ?)"#
    ))
    .bind(DbTime::now())
    .bind(guild_id)
    .bind(week)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Salon des messages de l'objectif : annonces, sinon tableau de bord.
async fn target_channel(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as(r#"SELECT "channelId", "dashboardChannelId" FROM "CotisationConfig" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    Ok(row.and_then(|(announce, dashboard)| announce.or(dashboard)))
}

/// Textes du rappel, découpés pour rester sous les limites Discord (longueur
/// et nombre de mentions autorisées) : (texte, membres mentionnés).
pub fn reminder_messages(header: &str, below: &[ProgressRow]) -> Vec<(String, Vec<String>)> {
    let mut messages = Vec::new();
    let mut text = header.to_owned();
    let mut users: Vec<String> = Vec::new();
    for row in below {
        let item = format!("\n- <@{}> — {} / {}", row.user_id, money(row.amount), money(row.target));
        if !users.is_empty() && (users.len() == MENTIONS_PER_MESSAGE || text.len() + item.len() > MAX_MESSAGE_LENGTH) {
            messages.push((std::mem::replace(&mut text, header.to_owned()), std::mem::take(&mut users)));
        }
        text.push_str(&item);
        users.push(row.user_id.clone());
    }
    if !users.is_empty() {
        messages.push((text, users));
    }
    messages
}

async fn send_reminder(ctx: &Ctx, guild_id: Id<GuildMarker>, week: DbTime) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(channel_id) = target_channel(&ctx.db, &guild).await?.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let Some(objective) = objective::get_objective(&ctx.db, &guild).await? else {
        return Ok(());
    };
    // Cache des membres pas encore rempli (redémarrage) : on retentera à la minute suivante.
    if !members_ready(ctx, guild_id) || !claim(&ctx.db, "lastReminderAt", &guild, week).await? {
        return Ok(());
    }
    let tracked = objective::tracked_members(ctx, guild_id).await?;
    let progress = objective::progress_for(&ctx.db, &guild, week, &objective, &tracked).await?;
    let below: Vec<ProgressRow> = progress.into_iter().filter(|r| !r.exempt && !r.reached).collect();
    let header = format!(
        "⏰ **Objectif de cotisation** : {} cette semaine. Encore en dessous avant ce soir minuit :",
        objective.describe()
    );
    for (text, users) in reminder_messages(&header, &below) {
        let mentions = Mentions::Explicit {
            users: users.iter().filter_map(|id| parse_id(id)).collect(),
            roles: Vec::new(),
            everyone: false,
        };
        messages::send(ctx, channel_id, &Outgoing::text(text).with_mentions(mentions)).await?;
    }
    tracing::info!(%guild_id, below = below.len(), "rappel de l'objectif de cotisation");
    Ok(())
}

/// Bilan d'une semaine écoulée (Components V2, mentions non notifiées).
pub fn recap_components(week: DbTime, objective: &Objective, progress: &[ProgressRow]) -> Vec<Component> {
    let tracked: Vec<&ProgressRow> = progress.iter().filter(|r| !r.exempt).collect();
    let reached: Vec<String> =
        tracked.iter().filter(|r| r.reached).map(|r| format!("- <@{}> — {}", r.user_id, money(r.amount))).collect();
    let missed: Vec<String> = tracked
        .iter()
        .filter(|r| !r.reached)
        .map(|r| format!("- <@{}> — {} / {}", r.user_id, money(r.amount), money(r.target)))
        .collect();
    let exempt: Vec<String> = progress.iter().filter(|r| r.exempt).map(|r| format!("<@{}>", r.user_id)).collect();
    let header = format!(
        "## 🎯 Bilan de l'objectif de cotisation\nSemaine {} · objectif {} · **{}/{}** l'ont atteint",
        objective::week_label(week),
        objective.describe(),
        reached.len(),
        tracked.len()
    );
    let mut blocks = vec![
        ListBlock {
            header: format!("### ✅ Atteint ({})", reached.len()),
            lines: reached,
            empty: "*Personne.*".into(),
        },
        ListBlock {
            header: format!("### ❌ Pas atteint ({})", missed.len()),
            lines: missed,
            empty: "*Personne. 🎉*".into(),
        },
    ];
    if !exempt.is_empty() {
        blocks.push(ListBlock {
            header: format!("### 🌙 Non concernés ({})", exempt.len()),
            lines: vec![dashboard::inline_list(&exempt, 600)],
            empty: String::new(),
        });
    }
    let budget = (V2_TEXT_LIMIT - 200).saturating_sub(header.len());
    let mut children = vec![text_display(header)];
    for block in pack_blocks(&blocks, budget) {
        children.push(separator());
        children.extend(text_displays(&block));
    }
    vec![container(Some(0x2ecc71), children)]
}

async fn send_recap(ctx: &Ctx, guild_id: Id<GuildMarker>, week: DbTime) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(channel_id) = target_channel(&ctx.db, &guild).await?.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let Some(objective) = objective::get_objective(&ctx.db, &guild).await? else {
        return Ok(());
    };
    if !members_ready(ctx, guild_id) || !claim(&ctx.db, "lastRecapAt", &guild, week).await? {
        return Ok(());
    }
    let previous = objective::previous_week(week);
    let tracked = objective::tracked_members(ctx, guild_id).await?;
    let progress = objective::progress_for(&ctx.db, &guild, previous, &objective, &tracked).await?;
    if progress.is_empty() {
        return Ok(());
    }
    // Mentions vides par défaut : les membres cités ne sont pas pingés.
    messages::send(ctx, channel_id, &Outgoing::components_v2(recap_components(previous, &objective, &progress)))
        .await?;
    tracing::info!(%guild_id, members = progress.len(), "bilan de l'objectif de cotisation");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        modules::cotisations::objective::Counts,
    };
    use chrono::TimeZone;

    fn row(user: u64, amount: i64, reached: bool, exempt: bool) -> ProgressRow {
        ProgressRow { user_id: (100_000_000_000_000_000 + user).to_string(), amount, target: 1000, reached, exempt }
    }

    #[test]
    fn due_times_in_paris() {
        // Hiver (UTC+1) : dimanche 17:00 UTC = 18:00 à Paris.
        assert!(reminder_due(Utc.with_ymd_and_hms(2026, 1, 18, 17, 0, 0).unwrap()));
        assert!(!reminder_due(Utc.with_ymd_and_hms(2026, 1, 18, 16, 59, 0).unwrap()));
        // Été (UTC+2) : lundi 07:00 UTC = 09:00 à Paris.
        assert!(recap_due(Utc.with_ymd_and_hms(2026, 8, 17, 7, 0, 0).unwrap()));
        assert!(!recap_due(Utc.with_ymd_and_hms(2026, 8, 17, 6, 59, 0).unwrap()));
        // Dimanche 23:30 UTC en hiver = lundi 00:30 à Paris : ni rappel, ni bilan.
        let night = Utc.with_ymd_and_hms(2026, 1, 18, 23, 30, 0).unwrap();
        assert!(!reminder_due(night) && !recap_due(night));
    }

    #[test]
    fn reminder_splits_mentions() {
        let below: Vec<ProgressRow> = (0..120).map(|i| row(i, 10, false, false)).collect();
        let messages = reminder_messages("Rappel :", &below);
        assert!(messages.len() > 1);
        assert!(messages.iter().all(|(text, users)| text.len() <= MAX_MESSAGE_LENGTH && users.len() <= 50));
        assert_eq!(messages.iter().map(|(_, users)| users.len()).sum::<usize>(), 120);
        assert!(messages[0].0.starts_with("Rappel :\n- <@100000000000000000> — 10"));
        assert!(reminder_messages("x", &[]).is_empty());
    }

    #[test]
    fn recap_groups_members() {
        let objective = Objective { amount: 1000, counts: Counts::Total, reminder: true };
        let progress = vec![row(1, 10, false, false), row(2, 2000, true, false), row(3, 0, false, true)];
        let components = recap_components(week_start(Utc::now()), &objective, &progress);
        let text = v2_text(&components);
        assert!(text.contains("**1/2** l'ont atteint"));
        assert!(text.contains("### ✅ Atteint (1)\n- <@100000000000000002>"));
        assert!(text.contains("### ❌ Pas atteint (1)\n- <@100000000000000001>"));
        assert!(text.contains("### 🌙 Non concernés (1)\n<@100000000000000003>"));
        assert!(v2_usage(&components).0 <= V2_TEXT_LIMIT);
    }

    #[tokio::test]
    async fn claims_once_per_week() {
        let dir = std::env::temp_dir().join(format!("cotisation-cron-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let week = week_start(Utc::now());
        objective::set_objective(&db, "g", &Objective { amount: 10, counts: Counts::Total, reminder: true })
            .await
            .unwrap();
        assert_eq!(pending(&db, "lastReminderAt", week).await.unwrap(), ["g"]);
        // Un objectif tout juste créé ne déclenche pas le bilan de la semaine passée.
        assert!(pending(&db, "lastRecapAt", week).await.unwrap().is_empty());
        assert!(claim(&db, "lastReminderAt", "g", week).await.unwrap());
        assert!(!claim(&db, "lastReminderAt", "g", week).await.unwrap());
        assert!(pending(&db, "lastReminderAt", week).await.unwrap().is_empty());
        objective::set_objective(&db, "g", &Objective { amount: 10, counts: Counts::Total, reminder: false })
            .await
            .unwrap();
        let next = objective::week_end(week);
        assert!(pending(&db, "lastReminderAt", next).await.unwrap().is_empty(), "rappel désactivé");
        assert_eq!(pending(&db, "lastRecapAt", next).await.unwrap(), ["g"]);
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
