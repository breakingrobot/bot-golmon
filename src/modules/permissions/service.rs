//! Lecture et écriture des réglages de permissions.

use super::{Level, Perm, Roles};
use crate::util::time::DbTime;
use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Rôle membre (réglé ici), gradé et lead (module recrues).
pub async fn roles(db: &SqlitePool, guild_id: &str) -> Result<Roles> {
    let recrues = crate::modules::recrues::service::get_config(db, guild_id).await?;
    Ok(Roles {
        member: member_role(db, guild_id).await?,
        grade: recrues.as_ref().and_then(|c| c.grade_role_id.clone()),
        lead: recrues.and_then(|c| c.lead_role_id),
    })
}

pub async fn member_role(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as(r#"SELECT "memberRoleId" FROM "PermissionConfig" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    Ok(row.and_then(|(role,)| role))
}

/// `None` : plus de rôle membre (le niveau « membre » ne laisse alors passer que les gradés).
pub async fn set_member_role(db: &SqlitePool, guild_id: &str, role_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "PermissionConfig" ("guildId", "memberRoleId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "memberRoleId" = excluded."memberRoleId", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(role_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Niveaux modifiés sur ce serveur (les autres actions gardent leur défaut).
/// Une valeur inconnue en base est ignorée.
pub async fn levels(db: &SqlitePool, guild_id: &str) -> Result<HashMap<String, Level>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as(r#"SELECT "action", "level" FROM "PermissionLevel" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().filter_map(|(action, level)| Some((action, Level::parse(&level)?))).collect())
}

/// `None` : retour au niveau par défaut.
pub async fn set_level(db: &SqlitePool, guild_id: &str, perm: Perm, level: Option<Level>) -> Result<()> {
    match level {
        Some(level) => {
            sqlx::query(
                r#"INSERT INTO "PermissionLevel" ("guildId", "action", "level", "updatedAt") VALUES (?, ?, ?, ?)
                   ON CONFLICT ("guildId", "action") DO UPDATE SET "level" = excluded."level", "updatedAt" = excluded."updatedAt""#,
            )
            .bind(guild_id)
            .bind(perm.key())
            .bind(level.as_str())
            .bind(DbTime::now())
            .execute(db)
            .await?;
        }
        None => {
            sqlx::query(r#"DELETE FROM "PermissionLevel" WHERE "guildId" = ? AND "action" = ?"#)
                .bind(guild_id)
                .bind(perm.key())
                .execute(db)
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn levels_round_trip() {
        let dir = std::env::temp_dir().join(format!("permission-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        set_member_role(&db, "g", Some("42")).await.unwrap();
        set_level(&db, "g", Perm::ContactAjouter, Some(Level::Grade)).await.unwrap();
        set_level(&db, "g", Perm::CoffreVoir, Some(Level::Lead)).await.unwrap();
        set_level(&db, "g", Perm::CoffreVoir, None).await.unwrap();
        let set = levels(&db, "g").await.unwrap();
        assert_eq!(set.get("contact.ajouter"), Some(&Level::Grade));
        assert!(!set.contains_key("coffre.voir"));
        assert_eq!(roles(&db, "g").await.unwrap().member.as_deref(), Some("42"));
        assert!(levels(&db, "autre").await.unwrap().is_empty());
        set_member_role(&db, "g", None).await.unwrap();
        assert!(member_role(&db, "g").await.unwrap().is_none());
    }
}
