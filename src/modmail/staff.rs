//! Prise en charge des tickets et statistiques du staff.
//!
//! Les prises en charge sont dans la base principale (`TicketClaim`) ; les
//! statistiques sont calculées depuis les messages des tickets.

use super::{Modmail, models::message_type, models::thread_status};
use crate::{context::Ctx, util::time::DbTime};
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Clone, Debug, sqlx::FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Claim {
    pub thread_id: String,
    pub staff_id: String,
    pub staff_name: String,
}

pub async fn claim_of(db: &SqlitePool, thread_id: &str) -> Result<Option<Claim>> {
    Ok(sqlx::query_as(r#"SELECT "threadId", "staffId", "staffName" FROM "TicketClaim" WHERE "threadId" = ?"#)
        .bind(thread_id)
        .fetch_optional(db)
        .await?)
}

pub async fn claims_of(db: &SqlitePool, thread_ids: &[String]) -> Result<HashMap<String, Claim>> {
    if thread_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; thread_ids.len()].join(", ");
    let sql =
        format!(r#"SELECT "threadId", "staffId", "staffName" FROM "TicketClaim" WHERE "threadId" IN ({placeholders})"#);
    let mut query = sqlx::query_as::<_, Claim>(&sql);
    for id in thread_ids {
        query = query.bind(id);
    }
    Ok(query.fetch_all(db).await?.into_iter().map(|c| (c.thread_id.clone(), c)).collect())
}

pub async fn set_claim(db: &SqlitePool, thread_id: &str, staff_id: &str, staff_name: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "TicketClaim" ("threadId", "staffId", "staffName", "claimedAt") VALUES (?, ?, ?, ?)
           ON CONFLICT ("threadId") DO UPDATE SET "staffId" = excluded."staffId", "staffName" = excluded."staffName",
           "claimedAt" = excluded."claimedAt""#,
    )
    .bind(thread_id)
    .bind(staff_id)
    .bind(staff_name)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn release_claim(db: &SqlitePool, thread_id: &str) -> Result<()> {
    sqlx::query(r#"DELETE FROM "TicketClaim" WHERE "threadId" = ?"#).bind(thread_id).execute(db).await?;
    Ok(())
}

/// Activité d'un membre du staff sur la période.
pub struct StaffActivity {
    pub name: String,
    pub replies: i64,
    pub tickets: i64,
    pub claims: i64,
    pub first_responses: i64,
    /// Délai moyen de première réponse, quand ce membre a répondu le premier.
    pub average_first_response: Option<String>,
}

/// Vue d'ensemble des tickets sur les 30 derniers jours.
pub struct TicketStats {
    pub open: i64,
    pub opened: i64,
    pub closed: i64,
    pub unanswered: i64,
    pub median_first_response: Option<String>,
    pub staff: Vec<StaffActivity>,
}

/// Durée lisible : « 12 min », « 3 h 05 », « 2 j 4 h ».
pub fn humanize_minutes(minutes: f64) -> String {
    let minutes = minutes.max(0.0).round() as i64;
    match minutes {
        m if m < 60 => format!("{m} min"),
        m if m < 1440 => format!("{} h {:02}", m / 60, m % 60),
        m => format!("{} j {} h", m / 1440, (m % 1440) / 60),
    }
}

impl Modmail {
    /// Tickets ouverts (accueil du site).
    pub async fn open_tickets(&self) -> Result<i64> {
        let (open,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM `threads` WHERE `status` = ?")
            .bind(thread_status::OPEN)
            .fetch_one(&self.db)
            .await?;
        Ok(open)
    }

    pub async fn ticket_stats(&self, ctx: &Ctx) -> Result<TicketStats> {
        let since = "datetime('now', '-30 days')";
        let (open,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM `threads` WHERE `status` = ?")
            .bind(thread_status::OPEN)
            .fetch_one(&self.db)
            .await?;
        let (opened,): (i64,) =
            sqlx::query_as(&format!("SELECT COUNT(*) FROM `threads` WHERE `created_at` >= {since}"))
                .fetch_one(&self.db)
                .await?;
        let (closed,): (i64,) =
            sqlx::query_as(&format!("SELECT COUNT(*) FROM `threads` WHERE `status` = ? AND `created_at` >= {since}"))
                .bind(thread_status::CLOSED)
                .fetch_one(&self.db)
                .await?;

        // Première réponse de chaque ticket récent : délai et auteur.
        let firsts: Vec<(String, f64)> = sqlx::query_as(&format!(
            "SELECT f.`user_id`, (julianday(f.`created_at`) - julianday(t.`created_at`)) * 1440
             FROM `threads` t
             JOIN (SELECT `thread_id`, MIN(`id`) AS mid FROM `thread_messages` WHERE `message_type` = ? GROUP BY `thread_id`) x
               ON x.`thread_id` = t.`id`
             JOIN `thread_messages` f ON f.`id` = x.mid
             WHERE t.`created_at` >= {since}"
        ))
        .bind(message_type::TO_USER)
        .fetch_all(&self.db)
        .await?;
        let mut delays: Vec<f64> = firsts.iter().map(|(_, m)| *m).collect();
        delays.sort_by(f64::total_cmp);
        let median_first_response = delays.get(delays.len() / 2).map(|m| humanize_minutes(*m));
        let unanswered = (opened - firsts.len() as i64).max(0);

        let replies: Vec<(String, String, i64, i64)> = sqlx::query_as(&format!(
            "SELECT `user_id`, MAX(`user_name`), COUNT(*), COUNT(DISTINCT `thread_id`) FROM `thread_messages`
             WHERE `message_type` = ? AND `user_id` IS NOT NULL AND `created_at` >= {since} GROUP BY `user_id`"
        ))
        .bind(message_type::TO_USER)
        .fetch_all(&self.db)
        .await?;
        let claims: Vec<(String, String, i64)> = sqlx::query_as(
            r#"SELECT "staffId", MAX("staffName"), COUNT(*) FROM "TicketClaim"
               WHERE "claimedAt" >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 days') GROUP BY "staffId""#,
        )
        .fetch_all(&ctx.db)
        .await?;

        let mut staff: HashMap<String, StaffActivity> = HashMap::new();
        let entry = |staff: &mut HashMap<String, StaffActivity>, id: &str, name: &str| {
            staff.entry(id.to_owned()).or_insert_with(|| StaffActivity {
                name: name.to_owned(),
                replies: 0,
                tickets: 0,
                claims: 0,
                first_responses: 0,
                average_first_response: None,
            });
        };
        for (id, name, count, tickets) in &replies {
            entry(&mut staff, id, name);
            let activity = staff.get_mut(id).expect("entrée créée");
            activity.replies = *count;
            activity.tickets = *tickets;
        }
        for (id, name, count) in &claims {
            entry(&mut staff, id, name);
            staff.get_mut(id).expect("entrée créée").claims = *count;
        }
        for (id, activity) in &mut staff {
            let mine: Vec<f64> = firsts.iter().filter(|(user, _)| user == id).map(|(_, m)| *m).collect();
            activity.first_responses = mine.len() as i64;
            if !mine.is_empty() {
                activity.average_first_response = Some(humanize_minutes(mine.iter().sum::<f64>() / mine.len() as f64));
            }
        }
        let mut staff: Vec<StaffActivity> = staff.into_values().collect();
        staff.sort_by(|a, b| b.replies.cmp(&a.replies).then(b.claims.cmp(&a.claims)));

        Ok(TicketStats { open, opened, closed, unanswered, median_first_response, staff })
    }
}

#[cfg(test)]
mod tests {
    use super::humanize_minutes;

    #[test]
    fn humanizes_delays() {
        assert_eq!(humanize_minutes(4.4), "4 min");
        assert_eq!(humanize_minutes(185.0), "3 h 05");
        assert_eq!(humanize_minutes(3000.0), "2 j 2 h");
    }
}
