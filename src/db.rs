use anyhow::{Context as _, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
use std::{path::Path, str::FromStr as _, time::Duration};

/// Ouvre une base SQLite en WAL (lectures concurrentes pendant les écritures),
/// clés étrangères actives pour les suppressions en cascade.
pub async fn open(path: &Path, max_connections: u32) -> Result<SqlitePool> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await.with_context(|| format!("création du dossier {}", parent.display()))?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(10));

    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .with_context(|| format!("ouverture de la base {}", path.display()))
}

pub async fn open_main(path: &Path) -> Result<SqlitePool> {
    let pool = open(path, 4).await?;
    sqlx::migrate!("./migrations").run(&pool).await.context("migrations de la base principale")?;
    Ok(pool)
}

/// Une violation de contrainte d'unicité est souvent un cas métier normal
/// (plaque ou numéro déjà pris) : l'appelant formule alors le message.
pub fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.is_unique_violation())
}

pub fn is_foreign_key_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.is_foreign_key_violation())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::{
            presences::{model::ResponseStatus, service as presences},
            recrues::{Vote, service as recrues},
        },
        util::time::{DbTime, week_start},
    };

    async fn temp_db() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("bot-test-{}", uuid::Uuid::new_v4()));
        let pool = open_main(&dir.join("bot.db")).await.unwrap();
        (pool, dir)
    }

    #[tokio::test]
    async fn presence_responses_and_stats_round_trip() {
        let (db, dir) = temp_db().await;
        let now = DbTime::now();
        sqlx::query(
            r#"INSERT INTO "Presence" ("id", "guildId", "channelId", "titre", "eventDate", "roleId", "createdById", "createdAt", "closedAt")
               VALUES ('p1', 'g1', 'c1', 'Soirée', ?, 'r1', 'u0', ?, ?)"#,
        )
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&db)
        .await
        .unwrap();

        assert!(presences::register_response(&db, "p1", "u1", ResponseStatus::Present, None).await.unwrap());
        // Même réponse : rien ne change.
        assert!(!presences::register_response(&db, "p1", "u1", ResponseStatus::Present, None).await.unwrap());
        assert!(presences::register_response(&db, "p1", "u1", ResponseStatus::Retard, Some(15)).await.unwrap());

        let responses = presences::responses(&db, "p1").await.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].status(), ResponseStatus::Retard);
        assert_eq!(responses[0].retard_minutes, Some(15));
        assert!(responses[0].changed);

        let presence = presences::get_guild_presence(&db, "g1", "p1").await.unwrap().unwrap();
        assert_eq!(presence.event_date.to_db_string(), now.to_db_string());
        assert!(presences::get_guild_presence(&db, "autre", "p1").await.unwrap().is_none());

        let stats = presences::user_stats(&db, "g1", "u1").await.unwrap();
        assert_eq!((stats.total_closed, stats.retard, stats.present), (1, 1, 0));
        assert_eq!(stats.recent_changes.len(), 1);
        let leaderboard = presences::leaderboard(&db, "g1").await.unwrap();
        assert_eq!(leaderboard[0].retard, 1);

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn recrue_votes_and_vetoes() {
        let (db, dir) = temp_db().await;
        let week = week_start(chrono::Utc::now());
        sqlx::query(
            r#"INSERT INTO "RecrueSession" ("id", "guildId", "weekStart", "createdAt") VALUES ('s1', 'g1', ?, ?)"#,
        )
        .bind(week)
        .bind(DbTime::now())
        .execute(&db)
        .await
        .unwrap();
        recrues::add_entry(&db, "s1", "g1", "u1").await.unwrap();
        recrues::add_entry(&db, "s1", "g1", "u1").await.unwrap();

        let (entry, session) = recrues::find_entry_in_current_session(&db, "g1", "u1").await.unwrap().unwrap();
        assert_eq!(session.week_start, week);
        recrues::cast_vote(&db, &entry.id, "v1", Vote::Pour, "Bien").await.unwrap();
        recrues::cast_vote(&db, &entry.id, "v1", Vote::Contre, "Finalement non").await.unwrap();
        recrues::set_veto(&db, &entry.id, "lead", "Motif").await.unwrap();

        let session = recrues::current_session(&db, "g1").await.unwrap().unwrap();
        assert_eq!(session.entries.len(), 1);
        assert_eq!(session.entries[0].votes.len(), 1);
        assert_eq!(session.entries[0].votes[0].choix, "CONTRE");
        assert_eq!(session.entries[0].veto_by_id.as_deref(), Some("lead"));
        assert!(recrues::get_entry(&db, "autre-guilde", &entry.id).await.unwrap().is_none());

        // Rappel du dimanche : réservé une seule fois par session.
        assert!(session.reminder_sent_at.is_none());
        assert!(recrues::claim_reminder(&db, "s1").await.unwrap());
        assert!(!recrues::claim_reminder(&db, "s1").await.unwrap());
        let session = recrues::current_session(&db, "g1").await.unwrap().unwrap();
        assert!(session.reminder_sent_at.is_some());

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn presence_report_gathers_window_and_claims_once() {
        use crate::modules::presences::report;
        use chrono::{TimeZone, Utc};

        let (db, dir) = temp_db().await;
        let at = |m: u32, d: u32, h: u32, min: u32| DbTime(Utc.with_ymd_and_hms(2026, m, d, h, min, 0).unwrap());
        // Fenêtre de 4 semaines du rapport du lundi 17/08/2026.
        let window = report::report_window(Utc.with_ymd_and_hms(2026, 8, 17, 7, 0, 0).unwrap());
        assert_eq!((window.alert_start, window.end), (at(7, 19, 22, 0), at(8, 16, 22, 0)));

        for (id, guild, date, closed) in [
            ("in0", "g1", at(7, 19, 22, 0), true), // lundi 20/07 00:00 à Paris : inclus
            ("in1", "g1", at(8, 12, 19, 0), true),
            ("edge", "g1", at(8, 16, 22, 0), true), // lundi 17/08 00:00 : semaine en cours, exclu
            ("open", "g1", at(8, 13, 19, 0), false),
            ("other", "g2", at(8, 12, 19, 0), true),
        ] {
            sqlx::query(
                r#"INSERT INTO "Presence" ("id", "guildId", "channelId", "titre", "eventDate", "roleId", "createdById", "createdAt", "closedAt")
                   VALUES (?, ?, 'c1', ?, ?, 'r1', 'u0', ?, ?)"#,
            )
            .bind(id)
            .bind(guild)
            .bind(format!("Titre {id}"))
            .bind(date)
            .bind(date)
            .bind(closed.then_some(date))
            .execute(&db)
            .await
            .unwrap();
        }
        for (presence, user, status) in [
            ("in1", "u1", ResponseStatus::Present),
            ("in1", "u2", ResponseStatus::Absent),
            ("open", "u1", ResponseStatus::Present),
            ("other", "u1", ResponseStatus::Present),
            ("edge", "u1", ResponseStatus::Retard),
        ] {
            assert!(presences::register_response(&db, presence, user, status, None).await.unwrap());
        }
        for (id, user, starts, ends, ended) in [
            ("a1", "u1", at(7, 24, 0, 0), at(8, 13, 0, 0), None),
            ("a2", "u2", at(8, 1, 0, 0), at(8, 30, 0, 0), Some(at(8, 5, 0, 0))), // terminée plus tôt
            ("a3", "u3", at(8, 10, 0, 0), at(8, 20, 0, 0), Some(at(8, 1, 0, 0))), // annulée avant son début
            ("a4", "u4", at(6, 1, 0, 0), at(7, 1, 0, 0), None),                  // avant la fenêtre
            ("a5", "u5", at(8, 20, 0, 0), at(8, 25, 0, 0), None),                // après la fenêtre
        ] {
            sqlx::query(
                r#"INSERT INTO "Absence" ("id", "guildId", "userId", "motif", "startsAt", "endsAt", "createdById", "endedAt")
                   VALUES (?, 'g1', ?, 'Vacances', ?, ?, 'u0', ?)"#,
            )
            .bind(id)
            .bind(user)
            .bind(starts)
            .bind(ends)
            .bind(ended)
            .execute(&db)
            .await
            .unwrap();
        }

        let data = report::gather(&db, "g1", window.alert_start, window.end).await.unwrap();
        let ids: Vec<&str> = data.presences.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["in0", "in1"]);
        assert_eq!(data.presences[1].titre, "Titre in1");
        assert_eq!(data.presences[1].event_date, at(8, 12, 19, 0));
        let responses: Vec<(&str, &str, &str)> =
            data.responses.iter().map(|r| (r.presence_id.as_str(), r.user_id.as_str(), r.status.as_str())).collect();
        assert_eq!(responses, vec![("in1", "u1", "PRESENT"), ("in1", "u2", "ABSENT")]);
        let absences: Vec<(&str, DbTime, DbTime)> =
            data.absences.iter().map(|a| (a.user_id.as_str(), a.starts_at, a.ends_at)).collect();
        assert_eq!(absences, vec![("u1", at(7, 24, 0, 0), at(8, 13, 0, 0)), ("u2", at(8, 1, 0, 0), at(8, 5, 0, 0))]);

        // Envoi réservé une seule fois par semaine, et seulement avec un salon.
        let this_week = week_start(Utc::now());
        assert!(!report::claim_report(&db, "g1", this_week).await.unwrap());
        presences::upsert_config(
            &db,
            "g1",
            presences::ConfigUpdate { report_channel_id: Some("c9".to_owned()), ..Default::default() },
        )
        .await
        .unwrap();
        let config = presences::get_config(&db, "g1").await.unwrap().unwrap();
        assert_eq!((config.report_channel_id.as_deref(), config.alert_threshold()), (Some("c9"), 50));
        presences::upsert_config(
            &db,
            "g1",
            presences::ConfigUpdate { alert_threshold: Some(70), ..Default::default() },
        )
        .await
        .unwrap();
        let config = presences::get_config(&db, "g1").await.unwrap().unwrap();
        assert_eq!((config.report_channel_id.as_deref(), config.alert_threshold()), (Some("c9"), 70));

        assert_eq!(report::pending_guilds(&db, this_week).await.unwrap(), vec!["g1".to_owned()]);
        assert!(report::claim_report(&db, "g1", this_week).await.unwrap());
        assert!(!report::claim_report(&db, "g1", this_week).await.unwrap());
        assert!(report::pending_guilds(&db, this_week).await.unwrap().is_empty());
        // La semaine suivante, l'envoi redevient possible.
        let next_week = DbTime(this_week.0 + chrono::Duration::weeks(1));
        assert!(report::claim_report(&db, "g1", next_week).await.unwrap());

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
