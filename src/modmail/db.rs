//! Base de données du module ticket, au schéma exact de modmailbot.
//!
//! Les migrations portent les mêmes noms que celles de modmailbot et sont
//! enregistrées dans la même table `knex_migrations` : une base `data.sqlite`
//! d'une installation modmailbot existante peut être reprise telle quelle (et
//! reste lisible par modmailbot).

use anyhow::{Context as _, Result};
use sqlx::{Executor as _, SqlitePool};
use std::path::Path;

struct Migration {
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "20171223203915_create_tables.js",
        sql: r#"
CREATE TABLE IF NOT EXISTS `threads` (`id` varchar(36) not null, `status` integer not null, `is_legacy` integer not null, `user_id` varchar(20) not null, `user_name` varchar(128) not null, `channel_id` varchar(20) null, `created_at` datetime not null, primary key (`id`));
CREATE INDEX IF NOT EXISTS `threads_status_index` on `threads` (`status`);
CREATE INDEX IF NOT EXISTS `threads_user_id_index` on `threads` (`user_id`);
CREATE UNIQUE INDEX IF NOT EXISTS `threads_channel_id_unique` on `threads` (`channel_id`);
CREATE INDEX IF NOT EXISTS `threads_created_at_index` on `threads` (`created_at`);
CREATE TABLE IF NOT EXISTS `thread_messages` (`id` integer not null primary key autoincrement, `thread_id` varchar(36) not null, `message_type` integer not null, `user_id` varchar(20) null, `user_name` varchar(128) not null, `body` text not null, `is_anonymous` integer not null, `dm_message_id` varchar(20) null, `created_at` datetime not null, foreign key(`thread_id`) references `threads`(`id`) on delete CASCADE);
CREATE INDEX IF NOT EXISTS `thread_messages_thread_id_index` on `thread_messages` (`thread_id`);
CREATE UNIQUE INDEX IF NOT EXISTS `thread_messages_dm_message_id_unique` on `thread_messages` (`dm_message_id`);
CREATE INDEX IF NOT EXISTS `thread_messages_created_at_index` on `thread_messages` (`created_at`);
CREATE TABLE IF NOT EXISTS `blocked_users` (`user_id` varchar(20) not null, `user_name` varchar(128) not null, `blocked_by` varchar(20) null, `blocked_at` datetime not null, primary key (`user_id`));
CREATE TABLE IF NOT EXISTS `snippets` (`trigger` varchar(32) not null, `body` text not null, `is_anonymous` integer not null, `created_by` varchar(20) null, `created_at` datetime not null, primary key (`trigger`));
"#,
    },
    Migration {
        name: "20180224235946_add_close_at_to_threads.js",
        sql: r#"
ALTER TABLE `threads` ADD COLUMN `scheduled_close_at` datetime null default NULL;
ALTER TABLE `threads` ADD COLUMN `scheduled_close_id` varchar(20) null default NULL;
ALTER TABLE `threads` ADD COLUMN `scheduled_close_name` varchar(128) null default NULL;
CREATE INDEX `threads_scheduled_close_at_index` on `threads` (`scheduled_close_at`);
"#,
    },
    Migration {
        name: "20180421161550_add_alert_id_to_threads.js",
        sql: "ALTER TABLE `threads` ADD COLUMN `alert_id` varchar(20) null default NULL;",
    },
    Migration {
        name: "20180920224224_remove_is_anonymous_from_snippets.js",
        sql: "ALTER TABLE `snippets` DROP COLUMN `is_anonymous`;",
    },
    Migration {
        name: "20190306204728_add_scheduled_close_silent_to_threads.js",
        sql: "ALTER TABLE `threads` ADD COLUMN `scheduled_close_silent` integer null;",
    },
    Migration {
        name: "20190306211534_add_scheduled_suspend_to_threads.js",
        sql: r#"
ALTER TABLE `threads` ADD COLUMN `scheduled_suspend_at` datetime null default NULL;
ALTER TABLE `threads` ADD COLUMN `scheduled_suspend_id` varchar(20) null default NULL;
ALTER TABLE `threads` ADD COLUMN `scheduled_suspend_name` varchar(128) null default NULL;
CREATE INDEX `threads_scheduled_suspend_at_index` on `threads` (`scheduled_suspend_at`);
"#,
    },
    Migration {
        name: "20190609161116_create_updates_table.js",
        sql: "CREATE TABLE IF NOT EXISTS `updates` (`available_version` varchar(16) null, `last_checked` datetime null);",
    },
    Migration {
        name: "20190609193213_add_expires_at_to_blocked_users.js",
        sql: "ALTER TABLE `blocked_users` ADD COLUMN `expires_at` datetime null;",
    },
    Migration {
        name: "20191206002418_add_number_to_thread_messages.js",
        sql: "ALTER TABLE `thread_messages` ADD COLUMN `message_number` integer null;",
    },
    Migration {
        name: "20191206011638_add_thread_message_id_to_thread_messages.js",
        sql: r#"
ALTER TABLE `thread_messages` ADD COLUMN `inbox_message_id` varchar(20) null;
CREATE UNIQUE INDEX `thread_messages_inbox_message_id_unique` on `thread_messages` (`inbox_message_id`);
"#,
    },
    Migration {
        name: "20191206180113_add_dm_channel_id_to_thread_messages.js",
        sql: "ALTER TABLE `thread_messages` ADD COLUMN `dm_channel_id` varchar(20) null;",
    },
    Migration {
        name: "20200813230319_separate_message_components.js",
        sql: r#"
ALTER TABLE `thread_messages` ADD COLUMN `role_name` varchar(255) null;
ALTER TABLE `thread_messages` ADD COLUMN `attachments` text null;
ALTER TABLE `thread_messages` ADD COLUMN `small_attachments` text null;
ALTER TABLE `thread_messages` ADD COLUMN `use_legacy_format` boolean null;
UPDATE `thread_messages` SET `use_legacy_format` = 1;
"#,
    },
    Migration {
        name: "20200814011007_add_next_message_number_to_threads.js",
        sql: "ALTER TABLE `threads` ADD COLUMN `next_message_number` integer default '1';",
    },
    Migration {
        name: "20200818214801_change_alert_id_to_alert_ids.js",
        sql: r#"
ALTER TABLE `threads` ADD COLUMN `alert_ids` text null;
UPDATE `threads` SET `alert_ids` = `alert_id`;
ALTER TABLE `threads` DROP COLUMN `alert_id`;
"#,
    },
    Migration {
        name: "20201003153122_add_log_storage_fields_to_threads.js",
        sql: r#"
ALTER TABLE `threads` ADD COLUMN `log_storage_type` varchar(255) null default NULL;
ALTER TABLE `threads` ADD COLUMN `log_storage_data` text null default NULL;
"#,
    },
    Migration {
        name: "20201012183935_add_metadata_field_to_threads.js",
        sql: "ALTER TABLE `threads` ADD COLUMN `metadata` text null default NULL;",
    },
    Migration {
        name: "20201012203622_add_metadata_field_to_thread_messages.js",
        sql: "ALTER TABLE `thread_messages` ADD COLUMN `metadata` text null default NULL;",
    },
    Migration {
        name: "20201019013831_fix_thread_messages_body_column_type.js",
        sql: r#"
ALTER TABLE `thread_messages` ADD COLUMN `temp_body` text;
UPDATE thread_messages SET temp_body = body;
ALTER TABLE `thread_messages` DROP COLUMN `body`;
ALTER TABLE `thread_messages` RENAME COLUMN `temp_body` TO `body`;
"#,
    },
    Migration {
        name: "20201021232940_create_moderator_role_overrides_table.js",
        sql: "CREATE TABLE IF NOT EXISTS `moderator_role_overrides` (`moderator_id` varchar(20), `thread_id` varchar(36) null default NULL, `role_id` varchar(20), primary key (`moderator_id`, `thread_id`));",
    },
    Migration {
        name: "20201101210234_add_thread_numbers.js",
        sql: r#"
ALTER TABLE `threads` ADD COLUMN `thread_number` integer;
CREATE UNIQUE INDEX `threads_thread_number_unique` on `threads` (`thread_number`);
"#,
    },
    Migration {
        name: "20201101213139_set_default_thread_number_values.js",
        sql: r#"
UPDATE `threads` SET `thread_number` = (
  SELECT numbered.rn FROM (SELECT `id`, ROW_NUMBER() OVER (ORDER BY `created_at` ASC) AS rn FROM `threads`) AS numbered
  WHERE numbered.`id` = `threads`.`id`
);
"#,
    },
    Migration {
        name: "20210120231512_fix_moderator_role_overrides_thread_id_nullability.js",
        sql: r#"
ALTER TABLE `moderator_role_overrides` RENAME TO `old_moderator_role_overrides`;
CREATE TABLE `moderator_role_overrides` (`id` integer not null primary key autoincrement, `moderator_id` varchar(20) not null, `thread_id` varchar(36) null default NULL, `role_id` varchar(20) not null);
CREATE UNIQUE INDEX `moderator_role_overrides_moderator_id_thread_id_unique` on `moderator_role_overrides` (`moderator_id`, `thread_id`);
INSERT INTO `moderator_role_overrides` (`moderator_id`, `thread_id`, `role_id`) SELECT `moderator_id`, `thread_id`, `role_id` FROM `old_moderator_role_overrides`;
DROP TABLE `old_moderator_role_overrides`;
"#,
    },
    Migration {
        name: "20210401225428_notes.js",
        sql: "CREATE TABLE IF NOT EXISTS `notes` (`user_id` varchar(20) null, `note` text null);",
    },
    Migration {
        name: "20220604000000_note_refactor.js",
        sql: r#"
DROP TABLE IF EXISTS `notes`;
CREATE TABLE `notes` (`id` integer not null primary key autoincrement, `user_id` varchar(20) null, `author_id` varchar(20) null, `body` text null, `created_at` datetime);
CREATE INDEX `notes_user_id_index` on `notes` (`user_id`);
CREATE INDEX `notes_author_id_index` on `notes` (`author_id`);
"#,
    },
];

pub async fn open(path: &Path) -> Result<SqlitePool> {
    let pool = crate::db::open(path, 2).await?;
    migrate(&pool).await.context("migrations de la base modmail")?;
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> Result<()> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS `knex_migrations` (`id` integer not null primary key autoincrement, `name` varchar(255), `batch` integer, `migration_time` datetime);
         CREATE TABLE IF NOT EXISTS `knex_migrations_lock` (`index` integer not null primary key autoincrement, `is_locked` integer);
         INSERT INTO `knex_migrations_lock` (`is_locked`) SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM `knex_migrations_lock`);",
    )
    .await?;

    let applied: Vec<(String,)> = sqlx::query_as("SELECT `name` FROM `knex_migrations`").fetch_all(pool).await?;
    let pending: Vec<&Migration> =
        MIGRATIONS.iter().filter(|m| !applied.iter().any(|(name,)| name == m.name)).collect();
    if pending.is_empty() {
        return Ok(());
    }

    let (batch,): (i64,) =
        sqlx::query_as("SELECT COALESCE(MAX(`batch`), 0) + 1 FROM `knex_migrations`").fetch_one(pool).await?;
    for migration in pending {
        let mut tx = pool.begin().await?;
        tx.execute(migration.sql).await.with_context(|| format!("migration {}", migration.name))?;
        sqlx::query("INSERT INTO `knex_migrations` (`name`, `batch`, `migration_time`) VALUES (?, ?, ?)")
            .bind(migration.name)
            .bind(batch)
            .bind(chrono::Utc::now().timestamp_millis())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(migration = migration.name, "migration modmail appliquée");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrates_fresh_database_to_modmailbot_schema() {
        let dir = std::env::temp_dir().join(format!("mm-test-{}", uuid::Uuid::new_v4()));
        let pool = open(&dir.join("data.sqlite")).await.unwrap();
        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('thread_messages') ORDER BY cid")
                .fetch_all(&pool)
                .await
                .unwrap();
        let names: Vec<&str> = columns.iter().map(|(n,)| n.as_str()).collect();
        for expected in [
            "id",
            "thread_id",
            "message_type",
            "user_id",
            "user_name",
            "is_anonymous",
            "dm_message_id",
            "created_at",
            "message_number",
            "inbox_message_id",
            "dm_channel_id",
            "role_name",
            "attachments",
            "small_attachments",
            "use_legacy_format",
            "metadata",
            "body",
        ] {
            assert!(names.contains(&expected), "colonne manquante : {expected}");
        }
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM knex_migrations").fetch_one(&pool).await.unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
        // Idempotent : une seconde ouverture n'applique rien.
        pool.close().await;
        let pool = open(&dir.join("data.sqlite")).await.unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM knex_migrations").fetch_one(&pool).await.unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
        let _ = std::fs::remove_dir_all(dir);
    }
}
