//! Accès aux tables modmailbot (`src/data/*.js`).

use super::{
    models::{BlockedUser, Note, Snippet, Thread, ThreadMessage, message_type, thread_status},
    util::now_db,
};
use anyhow::Result;
use serde_json::Value;
use sqlx::SqlitePool;

const THREAD_COLUMNS: &str = "`id`, `status`, `user_id`, `user_name`, `channel_id`, `created_at`, `scheduled_close_at`, `scheduled_close_id`, \
    `scheduled_close_name`, `scheduled_close_silent`, `scheduled_suspend_at`, `scheduled_suspend_name`, `alert_ids`, `log_storage_type`, \
    `log_storage_data`, `thread_number`";

fn thread_query(filter: &str) -> String {
    format!("SELECT {THREAD_COLUMNS} FROM `threads` WHERE {filter}")
}

pub async fn find_by_id(db: &SqlitePool, id: &str) -> Result<Option<Thread>> {
    Ok(sqlx::query_as(&thread_query("`id` = ? LIMIT 1")).bind(id).fetch_optional(db).await?)
}

pub async fn find_by_thread_number(db: &SqlitePool, number: i64) -> Result<Option<Thread>> {
    Ok(sqlx::query_as(&thread_query("`thread_number` = ? LIMIT 1")).bind(number).fetch_optional(db).await?)
}

pub async fn find_open_by_user_id(db: &SqlitePool, user_id: &str) -> Result<Option<Thread>> {
    Ok(sqlx::query_as(&thread_query("`user_id` = ? AND `status` = ? LIMIT 1"))
        .bind(user_id)
        .bind(thread_status::OPEN)
        .fetch_optional(db)
        .await?)
}

pub async fn find_by_channel_id(db: &SqlitePool, channel_id: &str) -> Result<Option<Thread>> {
    Ok(sqlx::query_as(&thread_query("`channel_id` = ? LIMIT 1")).bind(channel_id).fetch_optional(db).await?)
}

pub async fn find_by_channel_and_status(db: &SqlitePool, channel_id: &str, status: i64) -> Result<Option<Thread>> {
    Ok(sqlx::query_as(&thread_query("`channel_id` = ? AND `status` = ? LIMIT 1"))
        .bind(channel_id)
        .bind(status)
        .fetch_optional(db)
        .await?)
}

pub async fn closed_threads_by_user(db: &SqlitePool, user_id: &str) -> Result<Vec<Thread>> {
    Ok(sqlx::query_as(&thread_query("`status` = ? AND `user_id` = ? ORDER BY `created_at` DESC"))
        .bind(thread_status::CLOSED)
        .bind(user_id)
        .fetch_all(db)
        .await?)
}

pub async fn closed_thread_count(db: &SqlitePool, user_id: &str) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(`id`) FROM `threads` WHERE `status` = ? AND `user_id` = ?")
        .bind(thread_status::CLOSED)
        .bind(user_id)
        .fetch_one(db)
        .await?;
    Ok(count)
}

pub async fn open_threads(db: &SqlitePool) -> Result<Vec<Thread>> {
    Ok(sqlx::query_as(&thread_query("`status` = ?")).bind(thread_status::OPEN).fetch_all(db).await?)
}

pub async fn threads_to_close(db: &SqlitePool) -> Result<Vec<Thread>> {
    Ok(sqlx::query_as(&thread_query("`status` = ? AND `scheduled_close_at` IS NOT NULL AND `scheduled_close_at` <= ?"))
        .bind(thread_status::OPEN)
        .bind(now_db())
        .fetch_all(db)
        .await?)
}

pub async fn threads_to_suspend(db: &SqlitePool) -> Result<Vec<Thread>> {
    Ok(sqlx::query_as(&thread_query(
        "`status` = ? AND `scheduled_suspend_at` IS NOT NULL AND `scheduled_suspend_at` <= ?",
    ))
    .bind(thread_status::OPEN)
    .bind(now_db())
    .fetch_all(db)
    .await?)
}

/// Marqueur (colonne `metadata`, JSON) du message qui annonce une fermeture pour
/// inactivité. Le schéma modmailbot reste inchangé.
pub const AUTO_CLOSE_WARNING_METADATA: &str = r#"{"autoCloseWarning":true}"#;

/// Tickets ouverts inactifs depuis `cutoff` (date au format de la base) : dernier
/// échange avec l'utilisateur (`FROM_USER` ou `TO_USER`, à défaut la création du
/// ticket) antérieur à `cutoff`, aucune fermeture programmée (celle du staff n'est
/// jamais remplacée), et pas d'avertissement déjà donné depuis ce dernier échange.
pub async fn inactive_threads(db: &SqlitePool, cutoff: &str) -> Result<Vec<Thread>> {
    const LAST_EXCHANGE: &str = "COALESCE((SELECT MAX(m.`created_at`) FROM `thread_messages` m \
         WHERE m.`thread_id` = `threads`.`id` AND m.`message_type` IN (?2, ?3)), `threads`.`created_at`)";
    Ok(sqlx::query_as(&thread_query(&format!(
        "`status` = ?1 AND `scheduled_close_at` IS NULL AND {LAST_EXCHANGE} <= ?4 \
         AND NOT EXISTS (SELECT 1 FROM `thread_messages` w WHERE w.`thread_id` = `threads`.`id` \
             AND w.`metadata` = ?5 AND w.`created_at` >= {LAST_EXCHANGE})"
    )))
    .bind(thread_status::OPEN)
    .bind(message_type::FROM_USER)
    .bind(message_type::TO_USER)
    .bind(cutoff)
    .bind(AUTO_CLOSE_WARNING_METADATA)
    .fetch_all(db)
    .await?)
}

/// Crée un ticket et renvoie son identifiant (UUID v4, numéro incrémental).
pub async fn create_thread(db: &SqlitePool, user_id: &str, user_name: &str, channel_id: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = db.begin().await?;
    let (latest,): (Option<i64>,) =
        sqlx::query_as("SELECT MAX(`thread_number`) FROM `threads`").fetch_one(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO `threads` (`id`, `status`, `is_legacy`, `user_id`, `user_name`, `channel_id`, `next_message_number`, `created_at`, `thread_number`) \
         VALUES (?, ?, 0, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(thread_status::OPEN)
    .bind(user_id)
    .bind(user_name)
    .bind(channel_id)
    .bind(now_db())
    .bind(latest.unwrap_or(0) + 1)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn set_status(db: &SqlitePool, thread_id: &str, status: i64) -> Result<()> {
    sqlx::query("UPDATE `threads` SET `status` = ? WHERE `id` = ?").bind(status).bind(thread_id).execute(db).await?;
    Ok(())
}

pub async fn suspend(db: &SqlitePool, thread_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE `threads` SET `status` = ?, `scheduled_suspend_at` = NULL, `scheduled_suspend_id` = NULL, `scheduled_suspend_name` = NULL WHERE `id` = ?",
    )
    .bind(thread_status::SUSPENDED)
    .bind(thread_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn schedule_close(
    db: &SqlitePool,
    thread_id: &str,
    at: &str,
    user_id: &str,
    user_name: &str,
    silent: bool,
) -> Result<()> {
    sqlx::query(
        "UPDATE `threads` SET `scheduled_close_at` = ?, `scheduled_close_id` = ?, `scheduled_close_name` = ?, `scheduled_close_silent` = ? WHERE `id` = ?",
    )
    .bind(at)
    .bind(user_id)
    .bind(user_name)
    .bind(i64::from(silent))
    .bind(thread_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn cancel_scheduled_close(db: &SqlitePool, thread_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE `threads` SET `scheduled_close_at` = NULL, `scheduled_close_id` = NULL, `scheduled_close_name` = NULL, `scheduled_close_silent` = NULL WHERE `id` = ?",
    )
    .bind(thread_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn schedule_suspend(
    db: &SqlitePool,
    thread_id: &str,
    at: &str,
    user_id: &str,
    user_name: &str,
) -> Result<()> {
    sqlx::query("UPDATE `threads` SET `scheduled_suspend_at` = ?, `scheduled_suspend_id` = ?, `scheduled_suspend_name` = ? WHERE `id` = ?")
        .bind(at)
        .bind(user_id)
        .bind(user_name)
        .bind(thread_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn cancel_scheduled_suspend(db: &SqlitePool, thread_id: &str) -> Result<()> {
    sqlx::query("UPDATE `threads` SET `scheduled_suspend_at` = NULL, `scheduled_suspend_id` = NULL, `scheduled_suspend_name` = NULL WHERE `id` = ?")
        .bind(thread_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn alert_ids(db: &SqlitePool, thread_id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT `alert_ids` FROM `threads` WHERE `id` = ?").bind(thread_id).fetch_optional(db).await?;
    Ok(row.and_then(|(ids,)| ids))
}

async fn set_alert_ids(db: &SqlitePool, thread_id: &str, ids: Option<String>) -> Result<()> {
    sqlx::query("UPDATE `threads` SET `alert_ids` = ? WHERE `id` = ?").bind(ids).bind(thread_id).execute(db).await?;
    Ok(())
}

pub async fn add_alert(db: &SqlitePool, thread_id: &str, user_id: &str) -> Result<()> {
    let mut ids: Vec<String> =
        alert_ids(db, thread_id).await?.map(|s| s.split(',').map(str::to_owned).collect()).unwrap_or_default();
    if !ids.iter().any(|id| id == user_id) {
        ids.push(user_id.to_owned());
    }
    set_alert_ids(db, thread_id, Some(ids.join(","))).await
}

pub async fn remove_alert(db: &SqlitePool, thread_id: &str, user_id: &str) -> Result<()> {
    let Some(current) = alert_ids(db, thread_id).await? else {
        return Ok(());
    };
    let ids: Vec<&str> = current.split(',').filter(|id| *id != user_id).collect();
    set_alert_ids(db, thread_id, (!ids.is_empty()).then(|| ids.join(","))).await
}

pub async fn delete_alerts(db: &SqlitePool, thread_id: &str) -> Result<()> {
    set_alert_ids(db, thread_id, None).await
}

pub async fn update_log_storage(db: &SqlitePool, thread_id: &str, kind: &str, data: Option<&Value>) -> Result<()> {
    sqlx::query("UPDATE `threads` SET `log_storage_type` = ?, `log_storage_data` = ? WHERE `id` = ?")
        .bind(kind)
        .bind(data.map(Value::to_string))
        .bind(thread_id)
        .execute(db)
        .await?;
    Ok(())
}

// ---------- Messages ----------

/// Données d'un nouveau message de ticket.
#[derive(Default)]
pub struct NewMessage {
    pub message_type: i64,
    pub user_id: Option<String>,
    pub user_name: String,
    pub body: String,
    pub is_anonymous: bool,
    pub role_name: Option<String>,
    pub attachments: Vec<String>,
    pub small_attachments: Vec<String>,
    pub dm_message_id: Option<String>,
    pub dm_channel_id: Option<String>,
    pub inbox_message_id: Option<String>,
    pub metadata: Option<Value>,
}

/// Insère un message. Les réponses du staff reçoivent le numéro suivant du ticket.
pub async fn add_message(db: &SqlitePool, thread_id: &str, message: NewMessage) -> Result<ThreadMessage> {
    let mut tx = db.begin().await?;
    let number = if message.message_type == message_type::TO_USER {
        let (next,): (Option<i64>,) = sqlx::query_as("SELECT `next_message_number` FROM `threads` WHERE `id` = ?")
            .bind(thread_id)
            .fetch_one(&mut *tx)
            .await?;
        let next = next.unwrap_or(1);
        sqlx::query("UPDATE `threads` SET `next_message_number` = ? WHERE `id` = ?")
            .bind(next + 1)
            .bind(thread_id)
            .execute(&mut *tx)
            .await?;
        Some(next)
    } else {
        None
    };

    let json_list = |list: &[String]| serde_json::to_string(list).expect("liste sérialisable");
    let result = sqlx::query(
        "INSERT INTO `thread_messages` (`thread_id`, `message_type`, `message_number`, `user_id`, `user_name`, `role_name`, `body`, `is_anonymous`, \
         `attachments`, `small_attachments`, `dm_channel_id`, `dm_message_id`, `inbox_message_id`, `created_at`, `metadata`) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(message.message_type)
    .bind(number)
    .bind(&message.user_id)
    .bind(&message.user_name)
    .bind(&message.role_name)
    .bind(&message.body)
    .bind(i64::from(message.is_anonymous))
    .bind(json_list(&message.attachments))
    .bind(json_list(&message.small_attachments))
    .bind(&message.dm_channel_id)
    .bind(&message.dm_message_id)
    .bind(&message.inbox_message_id)
    .bind(now_db())
    .bind(message.metadata.as_ref().map(Value::to_string))
    .execute(&mut *tx)
    .await?;
    let id = result.last_insert_rowid();
    let row = sqlx::query_as("SELECT * FROM `thread_messages` WHERE `id` = ?").bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn set_message_ids(
    db: &SqlitePool,
    id: i64,
    dm_message_id: Option<&str>,
    inbox_message_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE `thread_messages` SET `dm_message_id` = COALESCE(?, `dm_message_id`), `inbox_message_id` = COALESCE(?, `inbox_message_id`) WHERE `id` = ?",
    )
    .bind(dm_message_id)
    .bind(inbox_message_id)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_message_attachments(db: &SqlitePool, id: i64, attachments: &[String]) -> Result<()> {
    sqlx::query("UPDATE `thread_messages` SET `attachments` = ? WHERE `id` = ?")
        .bind(serde_json::to_string(attachments)?)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_message_body(db: &SqlitePool, id: i64, body: &str) -> Result<()> {
    sqlx::query("UPDATE `thread_messages` SET `body` = ? WHERE `id` = ?").bind(body).bind(id).execute(db).await?;
    Ok(())
}

pub async fn delete_message(db: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM `thread_messages` WHERE `id` = ?").bind(id).execute(db).await?;
    Ok(())
}

pub async fn messages(db: &SqlitePool, thread_id: &str) -> Result<Vec<ThreadMessage>> {
    Ok(sqlx::query_as("SELECT * FROM `thread_messages` WHERE `thread_id` = ? ORDER BY `created_at` ASC, `id` ASC")
        .bind(thread_id)
        .fetch_all(db)
        .await?)
}

pub async fn message_by_dm_id(db: &SqlitePool, dm_message_id: &str) -> Result<Option<ThreadMessage>> {
    Ok(sqlx::query_as("SELECT * FROM `thread_messages` WHERE `dm_message_id` = ? LIMIT 1")
        .bind(dm_message_id)
        .fetch_optional(db)
        .await?)
}

/// Message d'un ticket correspondant à un message Discord (côté DM ou côté salon).
pub async fn message_for_discord_id(
    db: &SqlitePool,
    thread_id: &str,
    message_id: &str,
) -> Result<Option<ThreadMessage>> {
    Ok(sqlx::query_as(
        "SELECT * FROM `thread_messages` WHERE (`dm_message_id` = ? OR `inbox_message_id` = ?) AND `thread_id` = ? LIMIT 1",
    )
    .bind(message_id)
    .bind(message_id)
    .bind(thread_id)
    .fetch_optional(db)
    .await?)
}

pub async fn message_by_number(db: &SqlitePool, thread_id: &str, number: i64) -> Result<Option<ThreadMessage>> {
    Ok(sqlx::query_as("SELECT * FROM `thread_messages` WHERE `thread_id` = ? AND `message_number` = ? LIMIT 1")
        .bind(thread_id)
        .bind(number)
        .fetch_optional(db)
        .await?)
}

/// Dernier échange avec l'utilisateur (pour récupérer les messages manqués).
pub async fn latest_user_exchange(db: &SqlitePool, thread_id: &str) -> Result<Option<ThreadMessage>> {
    Ok(sqlx::query_as(
        "SELECT * FROM `thread_messages` WHERE `thread_id` = ? AND `message_type` IN (?, ?, ?) ORDER BY `created_at` DESC, `id` DESC LIMIT 1",
    )
    .bind(thread_id)
    .bind(message_type::FROM_USER)
    .bind(message_type::TO_USER)
    .bind(message_type::SYSTEM_TO_USER)
    .fetch_optional(db)
    .await?)
}

pub async fn update_chat_message(db: &SqlitePool, thread_id: &str, dm_message_id: &str, body: &str) -> Result<()> {
    sqlx::query("UPDATE `thread_messages` SET `body` = ? WHERE `thread_id` = ? AND `dm_message_id` = ?")
        .bind(body)
        .bind(thread_id)
        .bind(dm_message_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_chat_message(db: &SqlitePool, thread_id: &str, dm_message_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM `thread_messages` WHERE `thread_id` = ? AND `dm_message_id` = ?")
        .bind(thread_id)
        .bind(dm_message_id)
        .execute(db)
        .await?;
    Ok(())
}

// ---------- Blocages ----------

pub async fn block_status(db: &SqlitePool, user_id: &str) -> Result<Option<BlockedUser>> {
    Ok(sqlx::query_as("SELECT `user_id`, `blocked_by`, `expires_at` FROM `blocked_users` WHERE `user_id` = ? LIMIT 1")
        .bind(user_id)
        .fetch_optional(db)
        .await?)
}

pub async fn is_blocked(db: &SqlitePool, user_id: &str) -> Result<bool> {
    Ok(block_status(db, user_id).await?.is_some())
}

pub async fn block(
    db: &SqlitePool,
    user_id: &str,
    user_name: &str,
    blocked_by: &str,
    expires_at: Option<&str>,
) -> Result<()> {
    if is_blocked(db, user_id).await? {
        return Ok(());
    }
    sqlx::query("INSERT INTO `blocked_users` (`user_id`, `user_name`, `blocked_by`, `blocked_at`, `expires_at`) VALUES (?, ?, ?, ?, ?)")
        .bind(user_id)
        .bind(user_name)
        .bind(blocked_by)
        .bind(now_db())
        .bind(expires_at)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn unblock(db: &SqlitePool, user_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM `blocked_users` WHERE `user_id` = ?").bind(user_id).execute(db).await?;
    Ok(())
}

pub async fn update_block_expiry(db: &SqlitePool, user_id: &str, expires_at: &str) -> Result<()> {
    sqlx::query("UPDATE `blocked_users` SET `expires_at` = ? WHERE `user_id` = ?")
        .bind(expires_at)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn expired_blocks(db: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT `user_id` FROM `blocked_users` WHERE `expires_at` IS NOT NULL AND `expires_at` <= ?")
            .bind(now_db())
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn blocked_users(db: &SqlitePool) -> Result<Vec<BlockedUser>> {
    Ok(sqlx::query_as("SELECT `user_id`, `blocked_by`, `expires_at` FROM `blocked_users`").fetch_all(db).await?)
}

// ---------- Snippets ----------

pub async fn snippet(db: &SqlitePool, trigger: &str) -> Result<Option<Snippet>> {
    Ok(sqlx::query_as("SELECT `trigger`, `body` FROM `snippets` WHERE LOWER(`trigger`) = ? LIMIT 1")
        .bind(trigger.to_lowercase())
        .fetch_optional(db)
        .await?)
}

pub async fn add_snippet(db: &SqlitePool, trigger: &str, body: &str, created_by: &str) -> Result<()> {
    if snippet(db, trigger).await?.is_some() {
        return Ok(());
    }
    sqlx::query("INSERT INTO `snippets` (`trigger`, `body`, `created_by`, `created_at`) VALUES (?, ?, ?, ?)")
        .bind(trigger)
        .bind(body)
        .bind(created_by)
        .bind(now_db())
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_snippet(db: &SqlitePool, trigger: &str) -> Result<()> {
    sqlx::query("DELETE FROM `snippets` WHERE LOWER(`trigger`) = ?").bind(trigger.to_lowercase()).execute(db).await?;
    Ok(())
}

pub async fn all_snippets(db: &SqlitePool) -> Result<Vec<Snippet>> {
    Ok(sqlx::query_as("SELECT `trigger`, `body` FROM `snippets`").fetch_all(db).await?)
}

// ---------- Notes ----------

pub async fn notes_by_user(db: &SqlitePool, user_id: &str) -> Result<Vec<Note>> {
    Ok(sqlx::query_as("SELECT `id`, `user_id`, `author_id`, `body`, `created_at` FROM `notes` WHERE `user_id` = ?")
        .bind(user_id)
        .fetch_all(db)
        .await?)
}

pub async fn note(db: &SqlitePool, id: i64) -> Result<Option<Note>> {
    Ok(sqlx::query_as("SELECT `id`, `user_id`, `author_id`, `body`, `created_at` FROM `notes` WHERE `id` = ? LIMIT 1")
        .bind(id)
        .fetch_optional(db)
        .await?)
}

pub async fn delete_note(db: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM `notes` WHERE `id` = ?").bind(id).execute(db).await?;
    Ok(())
}

pub async fn create_note(db: &SqlitePool, user_id: &str, author_id: &str, body: &str) -> Result<()> {
    sqlx::query("INSERT INTO `notes` (`user_id`, `author_id`, `body`, `created_at`) VALUES (?, ?, ?, ?)")
        .bind(user_id)
        .bind(author_id)
        .bind(body)
        .bind(now_db())
        .execute(db)
        .await?;
    Ok(())
}

// ---------- Rôles affichés ----------

/// Rôle choisi par un modérateur, pour un ticket (`Some(thread)`) ou par défaut (`None`).
pub async fn role_override(db: &SqlitePool, moderator_id: &str, thread_id: Option<&str>) -> Result<Option<String>> {
    let row: Option<(String,)> = match thread_id {
        Some(thread) => {
            sqlx::query_as(
                "SELECT `role_id` FROM `moderator_role_overrides` WHERE `moderator_id` = ? AND `thread_id` = ? LIMIT 1",
            )
            .bind(moderator_id)
            .bind(thread)
            .fetch_optional(db)
            .await?
        }
        None => sqlx::query_as(
            "SELECT `role_id` FROM `moderator_role_overrides` WHERE `moderator_id` = ? AND `thread_id` IS NULL LIMIT 1",
        )
        .bind(moderator_id)
        .fetch_optional(db)
        .await?,
    };
    Ok(row.map(|(id,)| id))
}

pub async fn set_role_override(
    db: &SqlitePool,
    moderator_id: &str,
    thread_id: Option<&str>,
    role_id: &str,
) -> Result<()> {
    reset_role_override(db, moderator_id, thread_id).await?;
    sqlx::query("INSERT INTO `moderator_role_overrides` (`moderator_id`, `thread_id`, `role_id`) VALUES (?, ?, ?)")
        .bind(moderator_id)
        .bind(thread_id)
        .bind(role_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn reset_role_override(db: &SqlitePool, moderator_id: &str, thread_id: Option<&str>) -> Result<()> {
    match thread_id {
        Some(thread) => {
            sqlx::query("DELETE FROM `moderator_role_overrides` WHERE `moderator_id` = ? AND `thread_id` = ?")
                .bind(moderator_id)
                .bind(thread)
                .execute(db)
                .await?
        }
        None => {
            sqlx::query("DELETE FROM `moderator_role_overrides` WHERE `moderator_id` = ? AND `thread_id` IS NULL")
                .bind(moderator_id)
                .execute(db)
                .await?
        }
    };
    Ok(())
}

/// Ticket de la liste web, avec le nombre d'échanges.
#[derive(sqlx::FromRow)]
pub struct ThreadSummary {
    pub id: String,
    pub status: i64,
    pub user_id: String,
    pub user_name: String,
    pub created_at: String,
    pub thread_number: Option<i64>,
    pub exchanges: i64,
}

/// Recherche de tickets : identifiant ou nom d'utilisateur, numéro de ticket,
/// ou texte d'un message. `status` : `None` pour tous.
pub async fn search_threads(
    db: &SqlitePool,
    query: &str,
    status: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ThreadSummary>, i64)> {
    let query = query.trim();
    let pattern = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let filter = "(?1 = '' OR t.`user_id` = ?1 OR CAST(t.`thread_number` AS TEXT) = ltrim(?1, '#')
          OR t.`user_name` LIKE ?2 ESCAPE '\\'
          OR EXISTS (SELECT 1 FROM `thread_messages` m WHERE m.`thread_id` = t.`id`
                     AND m.`message_type` IN (?4, ?5, ?6) AND m.`body` LIKE ?2 ESCAPE '\\'))
         AND (?3 IS NULL OR t.`status` = ?3)";
    let (total,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM `threads` t WHERE {filter}"))
        .bind(query)
        .bind(&pattern)
        .bind(status)
        .bind(message_type::FROM_USER)
        .bind(message_type::TO_USER)
        .bind(message_type::CHAT)
        .fetch_one(db)
        .await?;
    let rows = sqlx::query_as(&format!(
        "SELECT t.`id`, t.`status`, t.`user_id`, t.`user_name`, t.`created_at`, t.`thread_number`,
                (SELECT COUNT(*) FROM `thread_messages` m WHERE m.`thread_id` = t.`id` AND m.`message_type` IN (?4, ?5)) AS exchanges
         FROM `threads` t WHERE {filter}
         ORDER BY t.`created_at` DESC LIMIT ?7 OFFSET ?8"
    ))
    .bind(query)
    .bind(&pattern)
    .bind(status)
    .bind(message_type::FROM_USER)
    .bind(message_type::TO_USER)
    .bind(message_type::CHAT)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;
    Ok((rows, total))
}
