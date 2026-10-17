//! Membres à ne jamais pinguer pour les présences, marqués absents d'office.

use super::model::BlocklistEntry;
use crate::util::{ids::new_id, time::DbTime};
use anyhow::Result;
use sqlx::SqlitePool;

pub async fn add(
    db: &SqlitePool,
    guild_id: &str,
    user_id: &str,
    raison: Option<&str>,
    added_by_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "PresenceBlocklist" ("id", "guildId", "userId", "raison", "addedById", "createdAt") VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT ("guildId", "userId") DO UPDATE SET "raison" = excluded."raison", "addedById" = excluded."addedById""#,
    )
    .bind(new_id())
    .bind(guild_id)
    .bind(user_id)
    .bind(raison)
    .bind(added_by_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "PresenceBlocklist" WHERE "guildId" = ? AND "userId" = ?"#)
        .bind(guild_id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list(db: &SqlitePool, guild_id: &str) -> Result<Vec<BlocklistEntry>> {
    Ok(sqlx::query_as(
        r#"SELECT "userId", "raison" FROM "PresenceBlocklist" WHERE "guildId" = ? ORDER BY "createdAt" ASC"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?)
}

pub async fn user_ids(db: &SqlitePool, guild_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(r#"SELECT "userId" FROM "PresenceBlocklist" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}
