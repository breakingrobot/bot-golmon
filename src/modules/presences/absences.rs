use super::{
    model::{Absence, ResponseStatus},
    service,
};
use crate::{
    context::Ctx,
    discord::messages::parse_id,
    util::{ids::new_id, time::DbTime},
};
use anyhow::Result;
use sqlx::SqlitePool;
use twilight_model::id::{Id, marker::GuildMarker};

const ABSENCE_COLUMNS: &str = r#""id", "guildId", "userId", "motif", "endsAt""#;

/// Résultat d'une modification du rôle d'absence. Le bot peut légitimement
/// échouer (permission « Gérer les rôles » absente, ou rôle plus haut que le
/// sien) : l'absence est tout de même enregistrée et le problème signalé.
pub struct RoleUpdate {
    pub ok: bool,
    pub reason: Option<String>,
}

impl RoleUpdate {
    fn ok() -> Self {
        Self { ok: true, reason: None }
    }

    fn failed(reason: impl Into<String>) -> Self {
        Self { ok: false, reason: Some(reason.into()) }
    }
}

async fn update_absence_role(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str, add: bool) -> Result<RoleUpdate> {
    let config = service::get_config(&ctx.db, &guild_id.to_string()).await?;
    let Some(role_id) = config.and_then(|c| c.absence_role_id).as_deref().and_then(parse_id) else {
        return Ok(RoleUpdate::failed("aucun rôle d'absence configuré"));
    };
    let Some(user_id) = parse_id(user_id) else {
        return Ok(RoleUpdate::failed("membre invalide"));
    };

    let has_role = match ctx.cache.member(guild_id, user_id) {
        Some(member) => member.roles().contains(&role_id),
        // Membre absent du cache : on tente quand même, l'API tranchera.
        None => !add,
    };

    let result = match (add, has_role) {
        (true, false) => ctx.http.add_guild_member_role(guild_id, user_id, role_id).await.map(drop),
        (false, true) => ctx.http.remove_guild_member_role(guild_id, user_id, role_id).await.map(drop),
        _ => Ok(()),
    };

    Ok(match result {
        Ok(()) => RoleUpdate::ok(),
        Err(error) => {
            tracing::warn!(%user_id, %error, add, "rôle d'absence impossible à modifier");
            RoleUpdate::failed(error.to_string())
        }
    })
}

pub async fn active_absence(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<Option<Absence>> {
    let now = DbTime::now();
    Ok(sqlx::query_as(&format!(
        r#"SELECT {ABSENCE_COLUMNS} FROM "Absence" WHERE "guildId" = ? AND "userId" = ? AND "endedAt" IS NULL AND "startsAt" <= ? AND "endsAt" > ? LIMIT 1"#
    ))
    .bind(guild_id)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_optional(db)
    .await?)
}

pub async fn list_active(db: &SqlitePool, guild_id: &str) -> Result<Vec<Absence>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {ABSENCE_COLUMNS} FROM "Absence" WHERE "guildId" = ? AND "endedAt" IS NULL AND "endsAt" > ? ORDER BY "endsAt" ASC"#
    ))
    .bind(guild_id)
    .bind(DbTime::now())
    .fetch_all(db)
    .await?)
}

pub struct NewAbsence {
    pub user_id: String,
    pub motif: String,
    pub starts_at: DbTime,
    pub ends_at: DbTime,
    pub created_by_id: String,
}

pub struct Declared {
    pub role: RoleUpdate,
    /// Présences ouvertes passées en absent.
    pub affected: usize,
}

/// Enregistre une absence, attribue le rôle si elle a déjà commencé et marque
/// le membre absent sur les présences ouvertes tombant dans la période.
pub async fn declare(ctx: &Ctx, guild_id: Id<GuildMarker>, absence: NewAbsence) -> Result<Declared> {
    sqlx::query(
        r#"INSERT INTO "Absence" ("id", "guildId", "userId", "motif", "startsAt", "endsAt", "createdById", "createdAt") VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(new_id())
    .bind(guild_id.to_string())
    .bind(&absence.user_id)
    .bind(&absence.motif)
    .bind(absence.starts_at)
    .bind(absence.ends_at)
    .bind(&absence.created_by_id)
    .bind(DbTime::now())
    .execute(&ctx.db)
    .await?;

    let role = if absence.starts_at <= DbTime::now() {
        update_absence_role(ctx, guild_id, &absence.user_id, true).await?
    } else {
        RoleUpdate::ok()
    };

    let presences: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "id" FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NULL AND "eventDate" >= ? AND "eventDate" <= ?"#,
    )
    .bind(guild_id.to_string())
    .bind(absence.starts_at)
    .bind(absence.ends_at)
    .fetch_all(&ctx.db)
    .await?;

    let mut affected = 0;
    for (presence_id,) in presences {
        if service::register_response(&ctx.db, &presence_id, &absence.user_id, ResponseStatus::Absent, None).await? {
            affected += 1;
            if let Err(error) = service::sync_message(ctx, &presence_id).await {
                tracing::error!(presence = %presence_id, ?error, "synchronisation de la présence après absence échouée");
            }
        }
    }

    Ok(Declared { role, affected })
}

/// Clôture une absence. Le rôle n'est retiré que si le membre n'a aucune
/// autre absence encore active (périodes qui se chevauchent).
pub async fn end(ctx: &Ctx, guild_id: Id<GuildMarker>, absence_id: &str, user_id: &str) -> Result<RoleUpdate> {
    let now = DbTime::now();
    let (still_absent,): (bool,) = sqlx::query_as(
        r#"SELECT EXISTS (SELECT 1 FROM "Absence" WHERE "guildId" = ? AND "userId" = ? AND "endedAt" IS NULL AND "id" != ? AND "startsAt" <= ? AND "endsAt" > ?)"#,
    )
    .bind(guild_id.to_string())
    .bind(user_id)
    .bind(absence_id)
    .bind(now)
    .bind(now)
    .fetch_one(&ctx.db)
    .await?;

    let role = if still_absent { RoleUpdate::ok() } else { update_absence_role(ctx, guild_id, user_id, false).await? };

    // Un rôle impossible à retirer laisse l'absence ouverte : le cron réessaiera.
    if role.ok {
        sqlx::query(r#"UPDATE "Absence" SET "endedAt" = ? WHERE "id" = ?"#)
            .bind(now)
            .bind(absence_id)
            .execute(&ctx.db)
            .await?;
    }
    Ok(role)
}

/// Absences arrivées à échéance, à clôturer par le cron.
pub async fn expired(db: &SqlitePool) -> Result<Vec<Absence>> {
    Ok(sqlx::query_as(&format!(r#"SELECT {ABSENCE_COLUMNS} FROM "Absence" WHERE "endedAt" IS NULL AND "endsAt" <= ?"#))
        .bind(DbTime::now())
        .fetch_all(db)
        .await?)
}

/// Absences en cours, dont le rôle doit être (ré)attribué.
pub async fn started(db: &SqlitePool) -> Result<Vec<Absence>> {
    let now = DbTime::now();
    Ok(sqlx::query_as(&format!(
        r#"SELECT {ABSENCE_COLUMNS} FROM "Absence" WHERE "endedAt" IS NULL AND "startsAt" <= ? AND "endsAt" > ?"#
    ))
    .bind(now)
    .bind(now)
    .fetch_all(db)
    .await?)
}

pub async fn apply_role(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> Result<RoleUpdate> {
    update_absence_role(ctx, guild_id, user_id, true).await
}
