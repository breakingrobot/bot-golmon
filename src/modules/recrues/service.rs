use super::embed::{build_components, build_dashboard};
use crate::{
    context::Ctx,
    discord::{
        members::{display_name, role_member_ids},
        messages::{self, Outgoing, parse_id},
    },
    util::{
        ids::new_id,
        time::{DbTime, week_start},
    },
};
use anyhow::Result;
use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct RecrueConfig {
    pub recrue_role_id: Option<String>,
    pub grade_role_id: Option<String>,
    pub lead_role_id: Option<String>,
    pub category_id: Option<String>,
    pub dashboard_channel_id: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub week_start: DbTime,
    pub closed_at: Option<DbTime>,
    pub message_id: Option<String>,
    /// Envoi du rappel des avis manquants (au plus un par session).
    pub reminder_sent_at: Option<DbTime>,
    #[sqlx(skip)]
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub session_id: String,
    pub user_id: String,
    pub veto_by_id: Option<String>,
    pub veto_motif: Option<String>,
    #[sqlx(skip)]
    pub votes: Vec<VoteRow>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct VoteRow {
    pub entry_id: String,
    pub voter_id: String,
    pub choix: String,
    pub motif: String,
}

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Option<RecrueConfig>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "RecrueConfig" WHERE "guildId" = ?"#).bind(guild_id).fetch_optional(db).await?)
}

#[derive(Default)]
pub struct ConfigUpdate {
    pub recrue_role_id: Option<String>,
    pub grade_role_id: Option<String>,
    pub lead_role_id: Option<String>,
    pub category_id: Option<String>,
    pub dashboard_channel_id: Option<String>,
}

impl ConfigUpdate {
    pub fn is_empty(&self) -> bool {
        self.recrue_role_id.is_none()
            && self.grade_role_id.is_none()
            && self.lead_role_id.is_none()
            && self.category_id.is_none()
            && self.dashboard_channel_id.is_none()
    }
}

pub async fn upsert_config(db: &SqlitePool, guild_id: &str, update: ConfigUpdate) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "RecrueConfig" ("guildId", "recrueRoleId", "gradeRoleId", "leadRoleId", "categoryId", "dashboardChannelId", "updatedAt")
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET
             "recrueRoleId" = COALESCE(excluded."recrueRoleId", "recrueRoleId"),
             "gradeRoleId" = COALESCE(excluded."gradeRoleId", "gradeRoleId"),
             "leadRoleId" = COALESCE(excluded."leadRoleId", "leadRoleId"),
             "categoryId" = COALESCE(excluded."categoryId", "categoryId"),
             "dashboardChannelId" = COALESCE(excluded."dashboardChannelId", "dashboardChannelId"),
             "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(update.recrue_role_id)
    .bind(update.grade_role_id)
    .bind(update.lead_role_id)
    .bind(update.category_id)
    .bind(update.dashboard_channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

async fn load_votes(db: &SqlitePool, entries: &mut [Entry]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; entries.len()].join(", ");
    let sql = format!(
        r#"SELECT "entryId", "voterId", "choix", "motif" FROM "RecrueVote" WHERE "entryId" IN ({placeholders}) ORDER BY "createdAt" ASC"#
    );
    let mut query = sqlx::query_as::<_, VoteRow>(&sql);
    for entry in entries.iter() {
        query = query.bind(&entry.id);
    }
    let votes = query.fetch_all(db).await?;
    for entry in entries.iter_mut() {
        entry.votes = votes.iter().filter(|v| v.entry_id == entry.id).cloned().collect();
    }
    Ok(())
}

const ENTRY_COLUMNS: &str = r#""id", "sessionId", "userId", "vetoById", "vetoMotif""#;

pub async fn get_session(db: &SqlitePool, session_id: &str) -> Result<Option<Session>> {
    let session: Option<Session> =
        sqlx::query_as(r#"SELECT * FROM "RecrueSession" WHERE "id" = ?"#).bind(session_id).fetch_optional(db).await?;
    let Some(mut session) = session else {
        return Ok(None);
    };
    session.entries = sqlx::query_as(&format!(
        r#"SELECT {ENTRY_COLUMNS} FROM "RecrueEntry" WHERE "sessionId" = ? ORDER BY "createdAt" ASC"#
    ))
    .bind(&session.id)
    .fetch_all(db)
    .await?;
    load_votes(db, &mut session.entries).await?;
    Ok(Some(session))
}

async fn current_session_id(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT "id" FROM "RecrueSession" WHERE "guildId" = ? AND "weekStart" = ?"#)
            .bind(guild_id)
            .bind(week_start(Utc::now()))
            .fetch_optional(db)
            .await?;
    Ok(row.map(|(id,)| id))
}

pub async fn current_session(db: &SqlitePool, guild_id: &str) -> Result<Option<Session>> {
    match current_session_id(db, guild_id).await? {
        Some(id) => get_session(db, &id).await,
        None => Ok(None),
    }
}

/// Garantit qu'une session existe pour la semaine en cours, en clôturant les
/// précédentes. Idempotent : appelable par le cron, une commande ou l'arrivée
/// d'une recrue sans risque de doublon. La nouvelle session est amorcée avec
/// les porteurs actuels du rôle recrue.
pub async fn ensure_current_session(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<Session> {
    let guild = guild_id.to_string();
    if let Some(id) = current_session_id(&ctx.db, &guild).await? {
        return get_session(&ctx.db, &id).await?.ok_or_else(|| anyhow::anyhow!("session {id} disparue"));
    }

    let config = get_config(&ctx.db, &guild).await?;
    let recrue_ids = config
        .and_then(|c| c.recrue_role_id)
        .as_deref()
        .and_then(parse_id)
        .map(|role| role_member_ids(ctx, guild_id, role))
        .unwrap_or_default();

    let now = DbTime::now();
    let session_id = new_id();
    let mut tx = ctx.db.begin().await?;
    // Les semaines écoulées sont conservées, seulement marquées closes.
    let week = week_start(Utc::now());
    sqlx::query(
        r#"UPDATE "RecrueSession" SET "closedAt" = ? WHERE "guildId" = ? AND "closedAt" IS NULL AND "weekStart" < ?"#,
    )
    .bind(now)
    .bind(&guild)
    .bind(week)
    .execute(&mut *tx)
    .await?;
    let inserted = sqlx::query(
        r#"INSERT INTO "RecrueSession" ("id", "guildId", "weekStart", "createdAt") VALUES (?, ?, ?, ?) ON CONFLICT ("guildId", "weekStart") DO NOTHING"#,
    )
    .bind(&session_id)
    .bind(&guild)
    .bind(week)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 1 {
        for user_id in &recrue_ids {
            sqlx::query(r#"INSERT INTO "RecrueEntry" ("id", "sessionId", "guildId", "userId", "createdAt") VALUES (?, ?, ?, ?, ?)"#)
                .bind(new_id())
                .bind(&session_id)
                .bind(&guild)
                .bind(user_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;

    if inserted.rows_affected() == 1 {
        tracing::info!(%guild_id, recrues = recrue_ids.len(), "nouvelle session de recrues");
    }
    // Une création concurrente a pu gagner la course : on relit la session de la semaine.
    let id = current_session_id(&ctx.db, &guild).await?.unwrap_or(session_id);
    get_session(&ctx.db, &id).await?.ok_or_else(|| anyhow::anyhow!("session {id} disparue"))
}

pub async fn add_entry(db: &SqlitePool, session_id: &str, guild_id: &str, user_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "RecrueEntry" ("id", "sessionId", "guildId", "userId", "createdAt") VALUES (?, ?, ?, ?, ?)
           ON CONFLICT ("sessionId", "userId") DO NOTHING"#,
    )
    .bind(new_id())
    .bind(session_id)
    .bind(guild_id)
    .bind(user_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_entry(db: &SqlitePool, session_id: &str, user_id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "RecrueEntry" WHERE "sessionId" = ? AND "userId" = ?"#)
        .bind(session_id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Un avis par gradé et par recrue : revoter remplace l'avis précédent.
pub async fn cast_vote(db: &SqlitePool, entry_id: &str, voter_id: &str, choix: super::Vote, motif: &str) -> Result<()> {
    let now = DbTime::now();
    sqlx::query(
        r#"INSERT INTO "RecrueVote" ("id", "entryId", "voterId", "choix", "motif", "createdAt", "updatedAt") VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT ("entryId", "voterId") DO UPDATE SET "choix" = excluded."choix", "motif" = excluded."motif", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(new_id())
    .bind(entry_id)
    .bind(voter_id)
    .bind(choix.as_str())
    .bind(motif)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_veto(db: &SqlitePool, entry_id: &str, lead_id: &str, motif: &str) -> Result<()> {
    sqlx::query(r#"UPDATE "RecrueEntry" SET "vetoById" = ?, "vetoMotif" = ?, "vetoAt" = ? WHERE "id" = ?"#)
        .bind(lead_id)
        .bind(motif)
        .bind(DbTime::now())
        .bind(entry_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn clear_veto(db: &SqlitePool, entry_id: &str) -> Result<()> {
    sqlx::query(r#"UPDATE "RecrueEntry" SET "vetoById" = NULL, "vetoMotif" = NULL, "vetoAt" = NULL WHERE "id" = ?"#)
        .bind(entry_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Recrue suivie, avec sa session. Scopée à la guilde : l'identifiant vient d'un
/// menu déroulant, donc du client.
pub async fn get_entry(db: &SqlitePool, guild_id: &str, entry_id: &str) -> Result<Option<(Entry, Session)>> {
    let entry: Option<Entry> =
        sqlx::query_as(&format!(r#"SELECT {ENTRY_COLUMNS} FROM "RecrueEntry" WHERE "id" = ? AND "guildId" = ?"#))
            .bind(entry_id)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    let Some(mut entry) = entry else {
        return Ok(None);
    };
    load_votes(db, std::slice::from_mut(&mut entry)).await?;
    let session: Session =
        sqlx::query_as(r#"SELECT * FROM "RecrueSession" WHERE "id" = ?"#).bind(&entry.session_id).fetch_one(db).await?;
    Ok(Some((entry, session)))
}

pub async fn find_entry_in_current_session(
    db: &SqlitePool,
    guild_id: &str,
    user_id: &str,
) -> Result<Option<(Entry, Session)>> {
    let Some(session_id) = current_session_id(db, guild_id).await? else {
        return Ok(None);
    };
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT "id" FROM "RecrueEntry" WHERE "sessionId" = ? AND "userId" = ?"#)
            .bind(&session_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    match row {
        Some((id,)) => get_entry(db, guild_id, &id).await,
        None => Ok(None),
    }
}

/// Semaines passées d'un membre, de la plus récente à la plus ancienne.
pub async fn user_history(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<Vec<(Entry, DbTime)>> {
    let rows: Vec<(String, DbTime)> = sqlx::query_as(
        r#"SELECT e."id", s."weekStart" FROM "RecrueEntry" e JOIN "RecrueSession" s ON s."id" = e."sessionId"
           WHERE e."guildId" = ? AND e."userId" = ? ORDER BY e."createdAt" DESC LIMIT 8"#,
    )
    .bind(guild_id)
    .bind(user_id)
    .fetch_all(db)
    .await?;
    let mut history = Vec::with_capacity(rows.len());
    for (id, week) in rows {
        if let Some((entry, _)) = get_entry(db, guild_id, &id).await? {
            history.push((entry, week));
        }
    }
    Ok(history)
}

pub async fn channel_id_of(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT "channelId" FROM "RecrueChannel" WHERE "guildId" = ? AND "userId" = ?"#)
            .bind(guild_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(|(id,)| id))
}

pub async fn remember_channel(db: &SqlitePool, guild_id: &str, user_id: &str, channel_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "RecrueChannel" ("id", "guildId", "userId", "channelId", "createdAt") VALUES (?, ?, ?, ?, ?)
           ON CONFLICT ("guildId", "userId") DO UPDATE SET "channelId" = excluded."channelId""#,
    )
    .bind(new_id())
    .bind(guild_id)
    .bind(user_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Réserve l'envoi du rappel de la session : `true` pour un seul appelant, même
/// en cas d'appels concurrents (le rappel part au plus une fois).
pub async fn claim_reminder(db: &SqlitePool, session_id: &str) -> Result<bool> {
    let result =
        sqlx::query(r#"UPDATE "RecrueSession" SET "reminderSentAt" = ? WHERE "id" = ? AND "reminderSentAt" IS NULL"#)
            .bind(DbTime::now())
            .bind(session_id)
            .execute(db)
            .await?;
    Ok(result.rows_affected() == 1)
}

/// Met à jour le message d'avis de la semaine. L'identifiant du message est
/// porté par la session : chaque semaine a son propre message, et ceux des
/// semaines écoulées restent visibles dans le salon comme archive.
pub async fn sync_dashboard(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<()> {
    ctx.events.publish(guild_id, crate::events::Topic::Recrues);
    let guild = guild_id.to_string();
    let Some(channel_id) =
        get_config(&ctx.db, &guild).await?.and_then(|c| c.dashboard_channel_id).as_deref().and_then(parse_id)
    else {
        return Ok(());
    };
    let Some(session) = current_session(&ctx.db, &guild).await? else {
        return Ok(());
    };

    // Les libellés de menus sont du texte brut (une mention n'y serait pas
    // rendue) : on résout les pseudos depuis le cache, repli sur l'identifiant.
    let labels: HashMap<String, String> = session
        .entries
        .iter()
        .filter_map(|e| {
            parse_id(&e.user_id).and_then(|id| display_name(ctx, guild_id, id)).map(|name| (e.user_id.clone(), name))
        })
        .collect();

    let message =
        Outgoing::embeds(vec![build_dashboard(&session)]).with_components(build_components(&session, &labels));
    let known = session.message_id.as_deref().and_then(parse_id);
    match messages::upsert(ctx, channel_id, known, &message).await {
        Ok(Some(new_id)) => {
            sqlx::query(r#"UPDATE "RecrueSession" SET "messageId" = ? WHERE "id" = ?"#)
                .bind(new_id.to_string())
                .bind(&session.id)
                .execute(&ctx.db)
                .await?;
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%guild_id, ?error, "salon des avis recrues inaccessible"),
    }
    Ok(())
}
