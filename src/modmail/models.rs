//! Lignes des tables modmailbot.

use serde_json::Value;
use sqlx::FromRow;

pub mod thread_status {
    pub const OPEN: i64 = 1;
    pub const CLOSED: i64 = 2;
    pub const SUSPENDED: i64 = 3;
}

pub mod message_type {
    pub const SYSTEM: i64 = 1;
    pub const CHAT: i64 = 2;
    pub const FROM_USER: i64 = 3;
    pub const TO_USER: i64 = 4;
    pub const LEGACY: i64 = 5;
    pub const COMMAND: i64 = 6;
    pub const SYSTEM_TO_USER: i64 = 7;
    pub const REPLY_EDITED: i64 = 8;
    pub const REPLY_DELETED: i64 = 9;
}

/// Messages qui n'ouvrent pas de ticket quand `ignoreAccidentalThreads` est actif.
pub const ACCIDENTAL_THREAD_MESSAGES: &[&str] = &[
    "ok",
    "okay",
    "thanks",
    "ty",
    "k",
    "kk",
    "thank you",
    "thanx",
    "thnx",
    "thx",
    "tnx",
    "ok thank you",
    "ok thanks",
    "ok ty",
    "ok thanx",
    "ok thnx",
    "ok thx",
    "ok no problem",
    "ok np",
    "okay thank you",
    "okay thanks",
    "okay ty",
    "okay thanx",
    "okay thnx",
    "okay thx",
    "okay no problem",
    "okay np",
    "okey thank you",
    "okey thanks",
    "okey ty",
    "okey thanx",
    "okey thnx",
    "okey thx",
    "okey no problem",
    "okey np",
    "cheers",
];

#[derive(Clone, Debug, FromRow)]
pub struct Thread {
    pub id: String,
    pub status: i64,
    pub user_id: String,
    pub user_name: String,
    pub channel_id: Option<String>,
    pub created_at: String,
    pub scheduled_close_at: Option<String>,
    pub scheduled_close_id: Option<String>,
    pub scheduled_close_name: Option<String>,
    pub scheduled_close_silent: Option<i64>,
    pub scheduled_suspend_at: Option<String>,
    pub scheduled_suspend_name: Option<String>,
    pub alert_ids: Option<String>,
    pub log_storage_type: Option<String>,
    pub log_storage_data: Option<String>,
    pub thread_number: Option<i64>,
}

impl Thread {
    pub fn is_open(&self) -> bool {
        self.status == thread_status::OPEN
    }

    pub fn is_closed(&self) -> bool {
        self.status == thread_status::CLOSED
    }

    pub fn number(&self) -> i64 {
        self.thread_number.unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default, FromRow)]
pub struct ThreadMessage {
    pub id: i64,
    pub thread_id: String,
    pub message_type: i64,
    pub message_number: Option<i64>,
    pub user_id: Option<String>,
    pub user_name: String,
    pub role_name: Option<String>,
    pub body: Option<String>,
    pub is_anonymous: i64,
    pub attachments: Option<String>,
    pub dm_channel_id: Option<String>,
    pub dm_message_id: Option<String>,
    pub inbox_message_id: Option<String>,
    pub created_at: String,
    pub use_legacy_format: Option<i64>,
    pub metadata: Option<String>,
}

impl ThreadMessage {
    pub fn body(&self) -> &str {
        self.body.as_deref().unwrap_or_default()
    }

    /// Les pièces jointes sont stockées en JSON (`["url", …]`).
    pub fn attachment_links(&self) -> Vec<String> {
        self.attachments.as_deref().and_then(|raw| serde_json::from_str(raw).ok()).unwrap_or_default()
    }

    pub fn metadata_value(&self, key: &str) -> Option<Value> {
        let metadata: Value = serde_json::from_str(self.metadata.as_deref()?).ok()?;
        metadata.get(key).cloned().filter(|v| !v.is_null())
    }

    pub fn is_from_user(&self) -> bool {
        self.message_type == message_type::FROM_USER
    }

    pub fn is_chat(&self) -> bool {
        self.message_type == message_type::CHAT
    }

    /// Message d'origine stocké dans les métadonnées des notifications
    /// d'édition et de suppression.
    pub fn original(&self) -> Option<ThreadMessage> {
        let value = self.metadata_value("originalThreadMessage")?;
        Some(ThreadMessage {
            user_id: value.get("user_id").and_then(json_string),
            user_name: value.get("user_name").and_then(json_string).unwrap_or_default(),
            body: value.get("body").and_then(json_string),
            message_number: value.get("message_number").and_then(Value::as_i64),
            ..Default::default()
        })
    }
}

pub fn json_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct Snippet {
    pub trigger: String,
    pub body: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct Note {
    pub id: i64,
    pub user_id: Option<String>,
    pub author_id: Option<String>,
    pub body: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct BlockedUser {
    pub user_id: String,
    pub blocked_by: Option<String>,
    pub expires_at: Option<String>,
}
