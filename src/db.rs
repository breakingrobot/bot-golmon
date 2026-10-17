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
        let dir = std::env::temp_dir().join(format!("golmon-test-{}", uuid::Uuid::new_v4()));
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

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
