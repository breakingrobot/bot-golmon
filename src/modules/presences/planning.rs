//! Présences récurrentes : un planning hebdomadaire (jour + heure, heure de
//! Paris) publie automatiquement la présence quelques heures avant l'événement.

use super::service::{self, NewPresence};
use crate::modules::audit::{self, Action, Entry};
use crate::{
    context::Ctx,
    discord::{
        embed::EmbedBuilder,
        interaction::{GUILD_ONLY, Ix, Reply, choice},
        messages::parse_id,
        options::Options,
    },
    util::{
        format::{DESCRIPTION_LIMIT, truncate},
        ids::new_id,
        time::{DbTime, TIMEZONE, parse_event_date},
    },
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::{FromRow, SqlitePool};
use twilight_util::builder::command::{IntegerBuilder, StringBuilder, SubCommandBuilder};

const WEEKDAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    pub guild_id: String,
    /// 1 = lundi … 7 = dimanche.
    pub weekday: i64,
    pub time: String,
    pub titre: String,
    pub description: Option<String>,
    pub lead_hours: i64,
    pub last_event_date: Option<DbTime>,
    pub created_by_id: String,
}

impl Schedule {
    fn weekday_label(&self) -> &'static str {
        WEEKDAYS.get((self.weekday - 1).clamp(0, 6) as usize).copied().unwrap_or("?")
    }

    fn summary(&self) -> String {
        format!("{} à {} — **{}** (publiée {} h avant)", self.weekday_label(), self.time, self.titre, self.lead_hours)
    }
}

/// Prochain événement du planning strictement après `now`.
fn next_event(weekday: i64, time: &str, now: DateTime<Utc>) -> Option<DbTime> {
    let today = now.with_timezone(&TIMEZONE).date_naive();
    (0..8).find_map(|offset| {
        let day = today + Duration::days(offset);
        if i64::from(day.weekday().number_from_monday()) != weekday {
            return None;
        }
        parse_event_date(&day.format("%d/%m/%Y").to_string(), time).filter(|date| date.0 > now)
    })
}

/// Événement à publier maintenant, s'il y en a un : dans la fenêtre de
/// publication et pas encore publié.
fn due_event(schedule: &Schedule, now: DateTime<Utc>) -> Option<DbTime> {
    let next = next_event(schedule.weekday, &schedule.time, now)?;
    let window_open = next.0 - Duration::hours(schedule.lead_hours) <= now;
    let already = schedule.last_event_date.is_some_and(|last| last >= next);
    (window_open && !already).then_some(next)
}

pub fn admin_subcommands() -> Vec<SubCommandBuilder> {
    vec![
        SubCommandBuilder::new("planifier", "Présence publiée automatiquement chaque semaine")
            .option(
                StringBuilder::new("jour", "Jour de l'événement")
                    .required(true)
                    .choices(WEEKDAYS.iter().enumerate().map(|(i, d)| (*d, (i + 1).to_string()))),
            )
            .option(StringBuilder::new("heure", "Heure de l'événement, ex: 21:00").required(true))
            .option(StringBuilder::new("titre", "Titre (défaut: Présence)"))
            .option(StringBuilder::new("description", "Description"))
            .option(
                IntegerBuilder::new("publication", "Publication, en heures avant l'événement (défaut: 24)")
                    .min_value(1)
                    .max_value(144),
            ),
        SubCommandBuilder::new("planning", "Liste les présences planifiées"),
        SubCommandBuilder::new("deplanifier", "Supprime une présence planifiée")
            .option(StringBuilder::new("planning", "Présence planifiée").required(true).autocomplete(true)),
    ]
}

async fn list(db: &SqlitePool, guild_id: &str) -> Result<Vec<Schedule>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "PresenceSchedule" WHERE "guildId" = ? ORDER BY "weekday", "time""#)
        .bind(guild_id)
        .fetch_all(db)
        .await?)
}

pub async fn handle_admin(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    match options.sub() {
        "planifier" => {
            let weekday = options.string("jour").and_then(|d| d.parse::<i64>().ok()).filter(|d| (1..=7).contains(d));
            let heure = options.string("heure").unwrap_or_default();
            // Heure validée et normalisée (« 21h » → « 21:00 ») via un événement de test.
            let time = (1..=7)
                .find_map(|day| next_event(day, &heure, Utc::now()))
                .map(|date| date.0.with_timezone(&TIMEZONE).format("%H:%M").to_string());
            let (Some(weekday), Some(time)) = (weekday, time) else {
                return ix.reply(Reply::ephemeral("Heure invalide. Format attendu : `HH:MM`.")).await;
            };
            let schedule = Schedule {
                id: new_id(),
                guild_id: guild.clone(),
                weekday,
                time,
                titre: options.string("titre").unwrap_or_else(|| "Présence".to_owned()),
                description: options.string("description"),
                lead_hours: options.integer("publication").unwrap_or(24),
                last_event_date: None,
                created_by_id: ix.user_id().to_string(),
            };
            sqlx::query(
                r#"INSERT INTO "PresenceSchedule" ("id", "guildId", "weekday", "time", "titre", "description", "leadHours", "createdById", "createdAt")
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(&schedule.id)
            .bind(&schedule.guild_id)
            .bind(schedule.weekday)
            .bind(&schedule.time)
            .bind(&schedule.titre)
            .bind(&schedule.description)
            .bind(schedule.lead_hours)
            .bind(&schedule.created_by_id)
            .bind(DbTime::now())
            .execute(&ix.ctx.db)
            .await?;
            let next = next_event(schedule.weekday, &schedule.time, Utc::now())
                .map(|d| {
                    format!(
                        "\nProchain événement : <t:{}:F>, publié <t:{}:R>.",
                        d.unix(),
                        (d.0 - Duration::hours(schedule.lead_hours)).timestamp()
                    )
                })
                .unwrap_or_default();
            audit::record(ix, Entry::new(Action::PresenceSchedule, None, schedule.summary())).await;
            ix.reply(Reply::ephemeral(format!("Présence planifiée : {}.{next}", schedule.summary()))).await
        }
        "planning" => {
            let schedules = list(&ix.ctx.db, &guild).await?;
            let description = if schedules.is_empty() {
                "Aucune présence planifiée. Ajoute-en une avec `/gestion presence planifier`.".to_owned()
            } else {
                truncate(
                    &schedules.iter().map(|s| format!("• {}", s.summary())).collect::<Vec<_>>().join("\n"),
                    DESCRIPTION_LIMIT,
                )
            };
            let embed =
                EmbedBuilder::new().title("🗓️ Présences planifiées").color(0x5865f2).description(description).build();
            ix.reply(Reply::embed(embed).hidden()).await
        }
        "deplanifier" => {
            let id = options.string("planning").unwrap_or_default();
            let deleted = sqlx::query(r#"DELETE FROM "PresenceSchedule" WHERE "id" = ? AND "guildId" = ?"#)
                .bind(&id)
                .bind(&guild)
                .execute(&ix.ctx.db)
                .await?
                .rows_affected();
            if deleted > 0 {
                audit::record(ix, Entry::new(Action::PresenceUnschedule, None, String::new())).await;
            }
            ix.reply(Reply::ephemeral(if deleted > 0 {
                "Présence planifiée supprimée. Les présences déjà publiées restent en place."
            } else {
                "Présence planifiée introuvable."
            }))
            .await
        }
        _ => Ok(()),
    }
}

pub async fn autocomplete(ix: &Ix) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    let choices = list(&ix.ctx.db, &guild_id.to_string())
        .await?
        .iter()
        .take(25)
        .map(|s| {
            let label = format!("{} {} — {}", s.weekday_label(), s.time, s.titre);
            choice(&crate::util::format::truncate(&label, 100), s.id.clone())
        })
        .collect();
    ix.autocomplete(choices).await
}

/// Publie les présences planifiées arrivées dans leur fenêtre de publication.
pub async fn sweep(ctx: &Ctx) {
    let schedules: Vec<Schedule> = match sqlx::query_as(r#"SELECT * FROM "PresenceSchedule""#).fetch_all(&ctx.db).await
    {
        Ok(rows) => rows,
        Err(error) => return tracing::error!(?error, "lecture du planning des présences"),
    };
    let now = Utc::now();
    for schedule in schedules {
        let Some(event_date) = due_event(&schedule, now) else { continue };
        if let Err(error) = publish(ctx, &schedule, event_date).await {
            tracing::error!(schedule = %schedule.id, ?error, "publication de la présence planifiée échouée");
        }
    }
}

async fn publish(ctx: &Ctx, schedule: &Schedule, event_date: DbTime) -> Result<()> {
    // Marquée avant publication : une erreur ne doit pas produire un doublon toutes les 5 minutes.
    sqlx::query(r#"UPDATE "PresenceSchedule" SET "lastEventDate" = ? WHERE "id" = ?"#)
        .bind(event_date)
        .bind(&schedule.id)
        .execute(&ctx.db)
        .await?;

    let Some(guild_id) = parse_id(&schedule.guild_id) else { return Ok(()) };
    if !ctx.config.is_guild_allowed(guild_id) {
        return Ok(());
    }
    let config = service::get_config(&ctx.db, &schedule.guild_id).await?;
    let Some(config) = config.filter(|c| c.default_role_id.is_some()) else {
        tracing::warn!(guild = %schedule.guild_id, "présence planifiée ignorée : module non configuré");
        return Ok(());
    };
    let Some(channel_id) = config.presence_channel_id.clone() else {
        tracing::warn!(guild = %schedule.guild_id, "présence planifiée ignorée : aucun salon des présences configuré");
        return Ok(());
    };
    let published = service::publish_presence(
        ctx,
        guild_id,
        NewPresence {
            channel_id,
            titre: schedule.titre.clone(),
            description: schedule.description.clone(),
            event_date,
            role_id: config.default_role_id.clone().unwrap_or_default(),
            absence_role_id: config.absence_role_id.clone(),
            created_by_id: schedule.created_by_id.clone(),
        },
    )
    .await?;
    if published.is_some() {
        tracing::info!(schedule = %schedule.id, "présence planifiée publiée");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn schedule(weekday: i64, time: &str, lead_hours: i64, last: Option<DbTime>) -> Schedule {
        Schedule {
            id: "s".into(),
            guild_id: "1".into(),
            weekday,
            time: time.into(),
            titre: "Présence".into(),
            description: None,
            lead_hours,
            last_event_date: last,
            created_by_id: "2".into(),
        }
    }

    #[test]
    fn finds_next_event_in_paris_time() {
        // Lundi 12 octobre 2026, 10:00 à Paris (08:00 UTC, heure d'été).
        let now = Utc.with_ymd_and_hms(2026, 10, 12, 8, 0, 0).unwrap();
        let wednesday = next_event(3, "21:00", now).unwrap();
        assert_eq!(wednesday.0, Utc.with_ymd_and_hms(2026, 10, 14, 19, 0, 0).unwrap());
        // Même jour, heure passée : semaine suivante.
        let monday = next_event(1, "09:00", now).unwrap();
        assert_eq!(monday.0, Utc.with_ymd_and_hms(2026, 10, 19, 7, 0, 0).unwrap());
        assert!(next_event(3, "25:00", now).is_none());
    }

    #[test]
    fn publishes_once_inside_window() {
        let now = Utc.with_ymd_and_hms(2026, 10, 12, 8, 0, 0).unwrap();
        // Mercredi 21:00 : fenêtre de 24 h pas encore ouverte lundi matin.
        assert!(due_event(&schedule(3, "21:00", 24, None), now).is_none());
        // Avec 72 h, elle l'est.
        let due = due_event(&schedule(3, "21:00", 72, None), now).unwrap();
        // Déjà publiée : rien à faire.
        assert!(due_event(&schedule(3, "21:00", 72, Some(due)), now).is_none());
    }
}
