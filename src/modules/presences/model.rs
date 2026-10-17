use crate::util::time::DbTime;
use sqlx::FromRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseStatus {
    Present,
    Absent,
    Retard,
}

impl ResponseStatus {
    pub const ALL: [Self; 3] = [Self::Present, Self::Absent, Self::Retard];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "PRESENT",
            Self::Absent => "ABSENT",
            Self::Retard => "RETARD",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Present => "Présent",
            Self::Absent => "Absent",
            Self::Retard => "Retard",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Present => "✅",
            Self::Absent => "❌",
            Self::Retard => "⏳",
        }
    }

    pub fn display(self) -> String {
        format!("{} {}", self.emoji(), self.label())
    }
}

pub const COLOR_ACTIVE: u32 = 0x5865f2;
pub const COLOR_CLOSED: u32 = 0x2c2f33;

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PresenceConfig {
    pub default_role_id: Option<String>,
    pub absence_role_id: Option<String>,
    pub presence_channel_id: Option<String>,
    pub reminder_channel_id: Option<String>,
    pub absence_channel_id: Option<String>,
    /// Salon du rapport hebdomadaire d'assiduité (absent : pas de rapport).
    pub report_channel_id: Option<String>,
    /// Seuil d'alerte du rapport, en % (défaut : [`DEFAULT_ALERT_THRESHOLD`]).
    pub alert_threshold: Option<i64>,
    pub last_report_at: Option<DbTime>,
}

pub const DEFAULT_ALERT_THRESHOLD: i64 = 50;

impl PresenceConfig {
    pub fn alert_threshold(&self) -> i64 {
        self.alert_threshold.unwrap_or(DEFAULT_ALERT_THRESHOLD)
    }
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Presence {
    pub id: String,
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: Option<String>,
    pub titre: String,
    pub description: Option<String>,
    pub event_date: DbTime,
    pub role_id: String,
    pub created_by_id: String,
    pub created_at: DbTime,
    pub closed_at: Option<DbTime>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PresenceResponse {
    pub user_id: String,
    pub status: String,
    pub retard_minutes: Option<i64>,
    /// Le membre a changé d'avis au moins une fois.
    pub changed: bool,
}

impl PresenceResponse {
    pub fn status(&self) -> ResponseStatus {
        ResponseStatus::parse(&self.status).unwrap_or(ResponseStatus::Absent)
    }
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct ResponseChange {
    pub user_id: String,
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_at: DbTime,
    pub titre: String,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Absence {
    pub id: String,
    pub guild_id: String,
    pub user_id: String,
    pub motif: String,
    pub ends_at: DbTime,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct BlocklistEntry {
    pub user_id: String,
    pub raison: Option<String>,
}
