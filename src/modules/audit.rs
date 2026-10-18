//! Journal des actions du staff : qui a forcé un vote, supprimé une
//! transaction, déclaré l'absence d'un autre membre, modifié la configuration…
//!
//! Chaque action est enregistrée en base (consultable sur le site) et publiée
//! dans le salon du journal s'il est configuré (`/config journal`). Le journal
//! ne doit jamais faire échouer l'action elle-même : les erreurs sont tracées.

use crate::{
    context::Ctx,
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply},
        messages::{self, Outgoing, parse_id},
        options::Options,
    },
    util::{format::truncate, time::DbTime},
};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};
use twilight_model::{
    channel::ChannelType,
    guild::Permissions,
    id::{Id, marker::GuildMarker},
};
use twilight_util::builder::command::{BooleanBuilder, ChannelBuilder, SubCommandBuilder};

/// Actions journalisées. Le code est stocké en base, le libellé affiché.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Config,
    PresenceForceVote,
    PresenceCancel,
    PresenceReset,
    PresenceSchedule,
    PresenceUnschedule,
    BlocklistAdd,
    BlocklistRemove,
    AbsenceDeclaredFor,
    AbsenceEndedFor,
    TransactionDelete,
    RecrueAdd,
    RecrueRemove,
    RecrueReset,
    VetoSet,
    VetoLifted,
    PhoneSetFor,
    PhoneRemovedFor,
    CotisationAddFor,
    CotisationDelete,
    ContactAdd,
    ContactUpdate,
    ContactDelete,
    ContactNote,
}

impl Action {
    pub const ALL: [Self; 24] = [
        Self::Config,
        Self::PresenceForceVote,
        Self::PresenceCancel,
        Self::PresenceReset,
        Self::PresenceSchedule,
        Self::PresenceUnschedule,
        Self::BlocklistAdd,
        Self::BlocklistRemove,
        Self::AbsenceDeclaredFor,
        Self::AbsenceEndedFor,
        Self::TransactionDelete,
        Self::RecrueAdd,
        Self::RecrueRemove,
        Self::RecrueReset,
        Self::VetoSet,
        Self::VetoLifted,
        Self::PhoneSetFor,
        Self::PhoneRemovedFor,
        Self::CotisationAddFor,
        Self::CotisationDelete,
        Self::ContactAdd,
        Self::ContactUpdate,
        Self::ContactDelete,
        Self::ContactNote,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::PresenceForceVote => "presence.forcevote",
            Self::PresenceCancel => "presence.cancel",
            Self::PresenceReset => "presence.reset",
            Self::PresenceSchedule => "presence.schedule",
            Self::PresenceUnschedule => "presence.unschedule",
            Self::BlocklistAdd => "blocklist.add",
            Self::BlocklistRemove => "blocklist.remove",
            Self::AbsenceDeclaredFor => "absence.declare",
            Self::AbsenceEndedFor => "absence.end",
            Self::TransactionDelete => "transaction.delete",
            Self::RecrueAdd => "recrue.add",
            Self::RecrueRemove => "recrue.remove",
            Self::RecrueReset => "recrue.reset",
            Self::VetoSet => "recrue.veto",
            Self::VetoLifted => "recrue.unveto",
            Self::PhoneSetFor => "numero.set",
            Self::PhoneRemovedFor => "numero.remove",
            Self::CotisationAddFor => "cotisation.add",
            Self::CotisationDelete => "cotisation.delete",
            Self::ContactAdd => "contact.add",
            Self::ContactUpdate => "contact.update",
            Self::ContactDelete => "contact.delete",
            Self::ContactNote => "contact.note",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.code() == code)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Config => "⚙️ Configuration modifiée",
            Self::PresenceForceVote => "🗳️ Vote forcé",
            Self::PresenceCancel => "🗑️ Présence annulée",
            Self::PresenceReset => "♻️ Statistiques de présence réinitialisées",
            Self::PresenceSchedule => "🗓️ Présence planifiée",
            Self::PresenceUnschedule => "🗓️ Planning supprimé",
            Self::BlocklistAdd => "🚫 Ajout à la blocklist",
            Self::BlocklistRemove => "✅ Retrait de la blocklist",
            Self::AbsenceDeclaredFor => "🌙 Absence déclarée pour un membre",
            Self::AbsenceEndedFor => "🌙 Absence clôturée pour un membre",
            Self::TransactionDelete => "🗑️ Transaction supprimée",
            Self::RecrueAdd => "🧑‍🎓 Recrue ajoutée",
            Self::RecrueRemove => "🧑‍🎓 Recrue retirée",
            Self::RecrueReset => "♻️ Nouvelle semaine de recrues forcée",
            Self::VetoSet => "⛔ Véto posé",
            Self::VetoLifted => "⛔ Véto levé",
            Self::PhoneSetFor => "📞 Numéro enregistré pour un membre",
            Self::PhoneRemovedFor => "📞 Numéro supprimé pour un membre",
            Self::CotisationAddFor => "💰 Cotisation déclarée pour un membre",
            Self::CotisationDelete => "💰 Cotisation supprimée",
            Self::ContactAdd => "📇 Contact ajouté",
            Self::ContactUpdate => "📇 Contact modifié",
            Self::ContactDelete => "📇 Contact supprimé",
            Self::ContactNote => "📝 Note ajoutée sur un contact",
        }
    }
}

/// Une action à journaliser.
pub struct Entry {
    pub action: Action,
    /// Membre visé, le cas échéant.
    pub target_id: Option<String>,
    /// Précisions lisibles (mentions en texte, jamais pingées).
    pub details: String,
}

impl Entry {
    pub fn new(action: Action, target_id: Option<String>, details: impl Into<String>) -> Self {
        Self { action, target_id, details: details.into() }
    }
}

/// Enregistre l'action faite par l'auteur de l'interaction.
pub async fn record(ix: &Ix, entry: Entry) {
    let Some(guild_id) = ix.guild_id() else { return };
    let actor_name = ix.interaction.member.as_ref().and_then(|m| m.nick.clone()).unwrap_or_else(|| {
        ix.user().map(|u| u.global_name.clone().unwrap_or_else(|| u.name.clone())).unwrap_or_default()
    });
    if let Err(error) = record_for(&ix.ctx, guild_id, &ix.user_id().to_string(), &actor_name, entry).await {
        tracing::warn!(?error, "journal du staff : enregistrement impossible");
    }
}

async fn record_for(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    actor_id: &str,
    actor_name: &str,
    entry: Entry,
) -> Result<()> {
    let details = truncate(&entry.details, 1000);
    sqlx::query(
        r#"INSERT INTO "AuditLog" ("guildId", "actorId", "actorName", "action", "targetId", "details", "createdAt")
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(guild_id.to_string())
    .bind(actor_id)
    .bind(actor_name)
    .bind(entry.action.code())
    .bind(&entry.target_id)
    .bind(&details)
    .bind(DbTime::now())
    .execute(&ctx.db)
    .await?;

    if let Some(channel_id) = audit_channel(&ctx.db, &guild_id.to_string()).await?.as_deref().and_then(parse_id) {
        let mut description = format!("Par <@{actor_id}>");
        if let Some(target) = &entry.target_id {
            description.push_str(&format!(" · membre visé : <@{target}>"));
        }
        if !details.is_empty() {
            description.push_str(&format!("\n{details}"));
        }
        let embed = EmbedBuilder::new()
            .title(entry.action.label())
            .color(0x5865f2)
            .description(description)
            .footer(footer(actor_name.to_owned()))
            .build();
        // Mentions affichées, jamais notifiées.
        messages::send(ctx, channel_id, &Outgoing::embeds(vec![embed])).await?;
    }
    Ok(())
}

async fn audit_channel(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as(r#"SELECT "auditChannelId" FROM "GuildSettings" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    Ok(row.and_then(|(channel,)| channel))
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct LogRow {
    pub actor_id: String,
    pub actor_name: String,
    pub action: String,
    pub target_id: Option<String>,
    pub details: String,
    pub created_at: DbTime,
}

/// Dernières entrées du journal, filtrées par action (code) si demandé.
pub async fn recent(
    db: &SqlitePool,
    guild_id: &str,
    action: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<LogRow>, i64)> {
    let (total,): (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "AuditLog" WHERE "guildId" = ?1 AND (?2 IS NULL OR "action" = ?2)"#)
            .bind(guild_id)
            .bind(action)
            .fetch_one(db)
            .await?;
    let rows = sqlx::query_as(
        r#"SELECT "actorId", "actorName", "action", "targetId", "details", "createdAt" FROM "AuditLog"
           WHERE "guildId" = ?1 AND (?2 IS NULL OR "action" = ?2) ORDER BY "id" DESC LIMIT ?3 OFFSET ?4"#,
    )
    .bind(guild_id)
    .bind(action)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?;
    Ok((rows, total))
}

/// `/config journal`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("journal", "Journal des actions du staff : salon de publication (sans option : affiche)")
        .option(ChannelBuilder::new("salon", "Salon où publier le journal").channel_types([ChannelType::GuildText]))
        .option(BooleanBuilder::new("desactiver", "Ne plus publier le journal sur Discord (il reste sur le site)"))
}

pub async fn config(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let guild = guild_id.to_string();
    let channel = options.channel("salon").map(|id| id.to_string());
    if channel.is_none() && options.boolean("desactiver") != Some(true) {
        let current = audit_channel(&ix.ctx.db, &guild).await?;
        return ix
            .reply(Reply::ephemeral(match current {
                Some(channel) => {
                    format!("Le journal du staff est publié dans <#{channel}>, et consultable sur le site.")
                }
                None => {
                    "Le journal du staff n'est publié dans aucun salon (il reste consultable sur le site).".to_owned()
                }
            }))
            .await;
    }
    sqlx::query(
        r#"INSERT INTO "GuildSettings" ("guildId", "auditChannelId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "auditChannelId" = excluded."auditChannelId", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(&guild)
    .bind(&channel)
    .bind(DbTime::now())
    .execute(&ix.ctx.db)
    .await?;
    ix.reply(Reply::ephemeral(match &channel {
        Some(channel) => format!("Le journal du staff sera publié dans <#{channel}>."),
        None => "Le journal du staff n'est plus publié sur Discord.".to_owned(),
    }))
    .await
}

#[cfg(test)]
mod tests {
    use super::{Action, recent};
    use crate::util::time::DbTime;

    #[tokio::test]
    async fn lists_and_filters_entries() {
        let dir = std::env::temp_dir().join(format!("audit-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        for (guild, action) in [("1", "presence.forcevote"), ("1", "config"), ("1", "config"), ("2", "config")] {
            sqlx::query(
                r#"INSERT INTO "AuditLog" ("guildId", "actorId", "actorName", "action", "details", "createdAt") VALUES (?, '9', 'Staff', ?, '', ?)"#,
            )
            .bind(guild)
            .bind(action)
            .bind(DbTime::now())
            .execute(&db)
            .await
            .unwrap();
        }
        let (rows, total) = recent(&db, "1", None, 10, 0).await.unwrap();
        assert_eq!((rows.len(), total), (3, 3));
        assert_eq!(rows[0].action, "config", "plus récentes d'abord");
        let (rows, total) = recent(&db, "1", Some("config"), 1, 0).await.unwrap();
        assert_eq!((rows.len(), total), (1, 2));
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn codes_are_unique_and_reversible() {
        for action in Action::ALL {
            assert_eq!(Action::from_code(action.code()), Some(action));
        }
        let mut codes: Vec<_> = Action::ALL.iter().map(|a| a.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), Action::ALL.len());
    }
}
