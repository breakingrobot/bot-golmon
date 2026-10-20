//! Réglages des rappels, choix des membres et rappels déjà envoyés.

use super::{Config, Kind, MemberPrefs};
use crate::{modules::presences::model::Presence, util::time::DbTime};
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Config> {
    let row: Option<(bool, i64, bool)> =
        sqlx::query_as(r#"SELECT "enabled", "presenceHours", "objective" FROM "RappelConfig" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    Ok(row.map_or_else(Config::default, |(enabled, presence_hours, objective)| Config {
        enabled,
        presence_hours,
        objective,
    }))
}

pub async fn set_config(db: &SqlitePool, guild_id: &str, config: &Config) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "RappelConfig" ("guildId", "enabled", "presenceHours", "objective", "updatedAt") VALUES (?, ?, ?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "enabled" = excluded."enabled", "presenceHours" = excluded."presenceHours",
             "objective" = excluded."objective", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(config.enabled)
    .bind(config.presence_hours)
    .bind(config.objective)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Serveurs où les rappels sont activés.
pub async fn enabled_guilds(db: &SqlitePool) -> Result<Vec<(String, Config)>> {
    let rows: Vec<(String, i64, bool)> =
        sqlx::query_as(r#"SELECT "guildId", "presenceHours", "objective" FROM "RappelConfig" WHERE "enabled" = 1"#)
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(guild, presence_hours, objective)| (guild, Config { enabled: true, presence_hours, objective }))
        .collect())
}

type PrefsRow = (String, bool, bool, Option<DbTime>);

fn prefs_of((_, presence_off, objective_off, dm_closed_at): PrefsRow) -> MemberPrefs {
    MemberPrefs { presence_off, objective_off, dm_closed_at }
}

/// Choix des membres d'un serveur (ceux qui n'ont rien réglé n'y sont pas).
pub async fn prefs(db: &SqlitePool, guild_id: &str) -> Result<HashMap<String, MemberPrefs>> {
    let rows: Vec<PrefsRow> = sqlx::query_as(
        r#"SELECT "userId", "presenceOff", "objectiveOff", "dmClosedAt" FROM "RappelMember" WHERE "guildId" = ?"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|row| (row.0.clone(), prefs_of(row))).collect())
}

pub async fn member_prefs(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<MemberPrefs> {
    let row: Option<PrefsRow> = sqlx::query_as(
        r#"SELECT "userId", "presenceOff", "objectiveOff", "dmClosedAt" FROM "RappelMember" WHERE "guildId" = ? AND "userId" = ?"#,
    )
    .bind(guild_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(prefs_of).unwrap_or_default())
}

/// Enregistre les choix d'un membre. Les réactiver efface aussi un ancien refus
/// de MP : le membre vient sans doute de rouvrir ses messages privés.
pub async fn set_prefs(db: &SqlitePool, guild_id: &str, user_id: &str, prefs: &MemberPrefs) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "RappelMember" ("guildId", "userId", "presenceOff", "objectiveOff", "dmClosedAt", "updatedAt") VALUES (?, ?, ?, ?, NULL, ?)
           ON CONFLICT ("guildId", "userId") DO UPDATE SET "presenceOff" = excluded."presenceOff",
             "objectiveOff" = excluded."objectiveOff", "dmClosedAt" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(user_id)
    .bind(prefs.presence_off)
    .bind(prefs.objective_off)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// MP refusé par Discord : pas de nouvel essai avant quelques jours.
pub async fn mark_dm_closed(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<()> {
    let now = DbTime::now();
    sqlx::query(
        r#"INSERT INTO "RappelMember" ("guildId", "userId", "dmClosedAt", "updatedAt") VALUES (?, ?, ?, ?)
           ON CONFLICT ("guildId", "userId") DO UPDATE SET "dmClosedAt" = excluded."dmClosedAt", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    Ok(())
}

/// Réserve un rappel avant de l'envoyer : `true` s'il n'était pas déjà parti.
pub async fn claim(db: &SqlitePool, guild_id: &str, kind: Kind, reference: &str, user_id: &str) -> Result<bool> {
    let result = sqlx::query(
        r#"INSERT OR IGNORE INTO "RappelSent" ("guildId", "kind", "ref", "userId", "sentAt") VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(guild_id)
    .bind(kind.as_str())
    .bind(reference)
    .bind(user_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Rappels de plus de 60 jours : inutiles pour le dédoublonnage.
pub async fn purge_sent(db: &SqlitePool) -> Result<()> {
    sqlx::query(r#"DELETE FROM "RappelSent" WHERE "sentAt" < ?"#)
        .bind(DbTime(DbTime::now().0 - chrono::Duration::days(60)))
        .execute(db)
        .await?;
    Ok(())
}

/// Présences ouvertes d'un serveur qui commencent avant `until`.
pub async fn upcoming_presences(db: &SqlitePool, guild_id: &str, until: DbTime) -> Result<Vec<Presence>> {
    Ok(sqlx::query_as(
        r#"SELECT * FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NULL AND "eventDate" > ? AND "eventDate" <= ?"#,
    )
    .bind(guild_id)
    .bind(DbTime::now())
    .bind(until)
    .fetch_all(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn config_prefs_and_dedupe() {
        let dir = std::env::temp_dir().join(format!("rappels-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        assert_eq!(get_config(&db, "g").await.unwrap(), Config::default(), "désactivé par défaut");
        assert!(enabled_guilds(&db).await.unwrap().is_empty());
        let config = Config { enabled: true, presence_hours: 3, objective: false };
        set_config(&db, "g", &config).await.unwrap();
        assert_eq!(get_config(&db, "g").await.unwrap(), config);
        assert_eq!(enabled_guilds(&db).await.unwrap(), [("g".to_owned(), config)]);

        // MP fermés, puis le membre coupe les présences : le refus est oublié.
        mark_dm_closed(&db, "g", "u").await.unwrap();
        assert!(member_prefs(&db, "g", "u").await.unwrap().dm_closed_at.is_some());
        let off = MemberPrefs { presence_off: true, ..Default::default() };
        set_prefs(&db, "g", "u", &off).await.unwrap();
        assert_eq!(member_prefs(&db, "g", "u").await.unwrap(), off);
        assert_eq!(prefs(&db, "g").await.unwrap().get("u"), Some(&off));
        assert_eq!(member_prefs(&db, "autre", "u").await.unwrap(), MemberPrefs::default());

        assert!(claim(&db, "g", Kind::Presence, "p1", "u").await.unwrap());
        assert!(!claim(&db, "g", Kind::Presence, "p1", "u").await.unwrap(), "une seule fois");
        assert!(claim(&db, "g", Kind::Objective, "p1", "u").await.unwrap());
        purge_sent(&db).await.unwrap();
        assert!(!claim(&db, "g", Kind::Presence, "p1", "u").await.unwrap(), "récents conservés");
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
