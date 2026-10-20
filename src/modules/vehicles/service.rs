use super::{api::normalize_plate, embed::build_dashboard};
use crate::{
    context::Ctx,
    db::is_unique_violation,
    discord::{
        messages::{self, Outgoing, parse_id},
        site::{self, Page},
    },
    modules::like_contains,
    util::{ids::new_id, time::DbTime},
};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Vehicle {
    pub plate: String,
    pub owner_id: String,
    pub owner_is_role: bool,
    pub modele: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct VehicleConfig {
    pub dashboard_channel_id: Option<String>,
    pub dashboard_message_id: Option<String>,
}

const COLUMNS: &str = r#""plate", "ownerId", "ownerIsRole", "modele""#;

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Option<VehicleConfig>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "VehicleConfig" WHERE "guildId" = ?"#).bind(guild_id).fetch_optional(db).await?)
}

pub async fn set_dashboard_channel(db: &SqlitePool, guild_id: &str, channel_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "VehicleConfig" ("guildId", "dashboardChannelId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "dashboardChannelId" = excluded."dashboardChannelId", "dashboardMessageId" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_by_plate(db: &SqlitePool, guild_id: &str, plate: &str) -> Result<Option<Vehicle>> {
    Ok(sqlx::query_as(&format!(r#"SELECT {COLUMNS} FROM "Vehicle" WHERE "guildId" = ? AND "plate" = ?"#))
        .bind(guild_id)
        .bind(normalize_plate(plate))
        .fetch_optional(db)
        .await?)
}

/// `Ok(false)` si la plaque a été enregistrée entre-temps (fenêtre ouverte par
/// la consultation de l'API).
pub async fn add(
    db: &SqlitePool,
    guild_id: &str,
    plate: &str,
    owner_id: &str,
    owner_is_role: bool,
    modele: Option<&str>,
) -> Result<bool> {
    let now = DbTime::now();
    let result = sqlx::query(
        r#"INSERT INTO "Vehicle" ("id", "guildId", "plate", "ownerId", "ownerIsRole", "modele", "createdAt", "updatedAt") VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(new_id())
    .bind(guild_id)
    .bind(normalize_plate(plate))
    .bind(owner_id)
    .bind(owner_is_role)
    .bind(modele)
    .bind(now)
    .bind(now)
    .execute(db)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(error) if is_unique_violation(&error) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub async fn set_owner(
    db: &SqlitePool,
    guild_id: &str,
    plate: &str,
    owner_id: &str,
    owner_is_role: bool,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "Vehicle" SET "ownerId" = ?, "ownerIsRole" = ?, "updatedAt" = ? WHERE "guildId" = ? AND "plate" = ?"#,
    )
    .bind(owner_id)
    .bind(owner_is_role)
    .bind(DbTime::now())
    .bind(guild_id)
    .bind(normalize_plate(plate))
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove(db: &SqlitePool, guild_id: &str, plate: &str) -> Result<()> {
    sqlx::query(r#"DELETE FROM "Vehicle" WHERE "guildId" = ? AND "plate" = ?"#)
        .bind(guild_id)
        .bind(normalize_plate(plate))
        .execute(db)
        .await?;
    Ok(())
}

pub async fn list(db: &SqlitePool, guild_id: &str) -> Result<Vec<Vehicle>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Vehicle" WHERE "guildId" = ? ORDER BY "ownerId" ASC, "plate" ASC"#
    ))
    .bind(guild_id)
    .fetch_all(db)
    .await?)
}

pub async fn search(db: &SqlitePool, guild_id: &str, query: &str) -> Result<Vec<Vehicle>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Vehicle" WHERE "guildId" = ? AND "plate" LIKE ? ESCAPE '\' ORDER BY "plate" ASC LIMIT 25"#
    ))
    .bind(guild_id)
    .bind(like_contains(&normalize_plate(query)))
    .fetch_all(db)
    .await?)
}

/// Met à jour (ou crée) le message unique listant les véhicules de la guilde.
pub async fn sync_dashboard(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let Some(config) = get_config(&ctx.db, guild_id).await? else {
        return Ok(());
    };
    let Some(channel_id) = config.dashboard_channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let mut components = build_dashboard(&list(&ctx.db, guild_id).await?);
    site::attach_page(ctx, guild_id, Page::Annuaire, &mut components);
    let message = Outgoing::components_v2(components);
    let known = config.dashboard_message_id.as_deref().and_then(parse_id);
    match messages::upsert(ctx, channel_id, known, &message).await {
        Ok(Some(new_id)) => {
            sqlx::query(r#"UPDATE "VehicleConfig" SET "dashboardMessageId" = ?, "updatedAt" = ? WHERE "guildId" = ?"#)
                .bind(new_id.to_string())
                .bind(DbTime::now())
                .bind(guild_id)
                .execute(&ctx.db)
                .await?;
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(error) => {
            tracing::warn!(guild = guild_id, ?error, "tableau de bord des véhicules inaccessible");
            Ok(())
        }
    }
}
