//! Fermeture des tickets inactifs (propre à ce bot, absent de modmailbot).
//!
//! Avec `autoCloseInactive`, un ticket ouvert sans échange avec l'utilisateur
//! depuis ce délai reçoit un avertissement et une fermeture programmée dans
//! `autoCloseWarning`. Une réponse de l'utilisateur ou du staff annule la
//! fermeture programmée (comme pour `!close <délai>`) : l'inactivité repart de
//! zéro. Rien n'est ajouté au schéma modmailbot : l'avertissement est repéré par
//! la colonne `metadata` du message système qui l'annonce.

use super::{
    Modmail, formatters,
    models::{Thread, ThreadMessage, message_type},
    store::{self, NewMessage},
    thread::Post,
    util::{DB_TIME_FORMAT, db_time_in, humanize_duration},
};
use crate::context::Ctx;
use anyhow::Result;

/// Nom affiché comme auteur de la fermeture programmée.
pub const AUTO_CLOSE_NAME: &str = "fermeture automatique";

/// Remplace `{inactive}` et `{delay}` dans le message d'avertissement.
pub fn warning_text(template: &str, inactive_ms: i64, warning_ms: i64) -> String {
    template
        .replace("{inactive}", &humanize_duration(inactive_ms, 2, None))
        .replace("{delay}", &humanize_duration(warning_ms, 2, None))
}

impl Modmail {
    /// `true` si la fermeture programmée du ticket vient de ce module.
    pub fn is_auto_close(ctx: &Ctx, thread: &Thread) -> bool {
        thread.scheduled_close_id.as_deref() == Some(ctx.bot_user_id.to_string().as_str())
    }

    /// Avertit les tickets devenus inactifs et programme leur fermeture.
    pub async fn warn_inactive_threads(&self, ctx: &Ctx) -> Result<()> {
        let Some(inactive_ms) = self.config.auto_close_inactive_ms else {
            return Ok(());
        };
        let warning_ms = self.config.auto_close_warning_ms;
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::milliseconds(inactive_ms)).format(DB_TIME_FORMAT).to_string();
        for thread in store::inactive_threads(&self.db, &cutoff).await? {
            // La fermeture est programmée d'abord : même si un envoi échoue, le
            // ticket n'est plus sélectionné au passage suivant.
            store::schedule_close(
                &self.db,
                &thread.id,
                &db_time_in(warning_ms),
                &ctx.bot_user_id.to_string(),
                AUTO_CLOSE_NAME,
                false,
            )
            .await?;

            let text = warning_text(&self.config.auto_close_message, inactive_ms, warning_ms);
            if let Err(error) = self.send_system_message_to_user(ctx, &thread, &text, true).await {
                tracing::warn!(user = %thread.user_id, ?error, "avertissement d'inactivité non délivré");
            }

            let notice = format!(
                "Ticket inactif : fermeture automatique dans {} sauf nouvelle réponse. `{}close cancel` pour l'annuler.",
                humanize_duration(warning_ms, 2, None),
                self.config.prefix
            );
            let content =
                formatters::system_thread_message(&ThreadMessage { body: Some(notice.clone()), ..Default::default() });
            let posted = self.post_to_thread(ctx, &thread, Post::text(content)).await.unwrap_or_else(|error| {
                tracing::warn!(user = %thread.user_id, ?error, "annonce de fermeture automatique non publiée");
                None
            });
            store::add_message(
                &self.db,
                &thread.id,
                NewMessage {
                    message_type: message_type::SYSTEM,
                    body: notice,
                    inbox_message_id: posted.map(|m| m.id.to_string()),
                    metadata: serde_json::from_str(store::AUTO_CLOSE_WARNING_METADATA).ok(),
                    ..Default::default()
                },
            )
            .await?;
            tracing::info!(user = %thread.user_id, thread = thread.number(), "ticket inactif : fermeture programmée");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modmail::{db, models::thread_status, util::now_db};
    use sqlx::SqlitePool;

    async fn insert_thread(db: &SqlitePool, id: &str, created_at: &str) {
        sqlx::query(
            "INSERT INTO `threads` (`id`, `status`, `is_legacy`, `user_id`, `user_name`, `channel_id`, `created_at`, `thread_number`) \
             VALUES (?, ?, 0, ?, 'membre', ?, ?, (SELECT COALESCE(MAX(`thread_number`), 0) + 1 FROM `threads`))",
        )
        .bind(id)
        .bind(thread_status::OPEN)
        .bind(format!("user-{id}"))
        .bind(format!("channel-{id}"))
        .bind(created_at)
        .execute(db)
        .await
        .unwrap();
    }

    async fn insert_message(db: &SqlitePool, thread_id: &str, kind: i64, created_at: &str, metadata: Option<&str>) {
        sqlx::query(
            "INSERT INTO `thread_messages` (`thread_id`, `message_type`, `user_name`, `body`, `is_anonymous`, `created_at`, `metadata`) \
             VALUES (?, ?, 'x', 'x', 0, ?, ?)",
        )
        .bind(thread_id)
        .bind(kind)
        .bind(created_at)
        .bind(metadata)
        .execute(db)
        .await
        .unwrap();
    }

    fn ago(days: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::days(days)).format(DB_TIME_FORMAT).to_string()
    }

    async fn selected(db: &SqlitePool) -> Vec<String> {
        let mut ids: Vec<String> =
            store::inactive_threads(db, &ago(7)).await.unwrap().into_iter().map(|t| t.id).collect();
        ids.sort();
        ids
    }

    #[tokio::test]
    async fn selects_only_inactive_threads_without_scheduled_close() {
        let dir = std::env::temp_dir().join(format!("mm-autoclose-{}", uuid::Uuid::new_v4()));
        let pool = db::open(&dir.join("data.sqlite")).await.unwrap();

        // Dernier échange il y a 10 jours ; le message système récent ne compte pas.
        insert_thread(&pool, "inactive", &ago(20)).await;
        insert_message(&pool, "inactive", message_type::FROM_USER, &ago(12), None).await;
        insert_message(&pool, "inactive", message_type::TO_USER, &ago(10), None).await;
        insert_message(&pool, "inactive", message_type::SYSTEM, &ago(1), None).await;
        insert_message(&pool, "inactive", message_type::CHAT, &ago(1), None).await;
        // Aucun message : la date de création fait foi.
        insert_thread(&pool, "silent", &ago(9)).await;
        // Ancien ticket, mais l'utilisateur a écrit hier.
        insert_thread(&pool, "active", &ago(30)).await;
        insert_message(&pool, "active", message_type::FROM_USER, &ago(1), None).await;
        // Ticket récent sans message.
        insert_thread(&pool, "recent", &now_db()).await;
        // Inactif, mais une fermeture est déjà programmée par le staff.
        insert_thread(&pool, "scheduled", &ago(30)).await;
        store::schedule_close(&pool, "scheduled", &now_db(), "staff", "Modo", false).await.unwrap();
        // Inactif, mais fermé.
        insert_thread(&pool, "closed", &ago(30)).await;
        store::set_status(&pool, "closed", thread_status::CLOSED).await.unwrap();

        assert_eq!(selected(&pool).await, vec!["inactive", "silent"]);

        // Après avertissement puis `!close cancel` : pas de second avertissement…
        insert_message(&pool, "silent", message_type::SYSTEM, &ago(2), Some(store::AUTO_CLOSE_WARNING_METADATA)).await;
        assert_eq!(selected(&pool).await, vec!["inactive"]);
        // … jusqu'à un nouvel échange suivi d'une nouvelle période d'inactivité.
        insert_message(&pool, "silent", message_type::TO_USER, &ago(1), None).await;
        assert_eq!(selected(&pool).await, vec!["inactive"]);
        sqlx::query(
            "UPDATE `thread_messages` SET `created_at` = ? WHERE `thread_id` = 'silent' AND `message_type` = ?",
        )
        .bind(ago(8))
        .bind(message_type::TO_USER)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE `thread_messages` SET `created_at` = ? WHERE `thread_id` = 'silent' AND `metadata` IS NOT NULL",
        )
        .bind(ago(8 + 1))
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(selected(&pool).await, vec!["inactive", "silent"]);

        pool.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fills_warning_template() {
        let text = warning_text("Inactif depuis {inactive}, fermeture dans {delay}.", 7 * 86_400_000, 86_400_000);
        assert_eq!(text, "Inactif depuis 1 semaine, fermeture dans 1 jour.");
    }
}
