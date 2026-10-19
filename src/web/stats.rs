//! Statistiques d'un serveur : assiduité aux présences, recrues de la semaine,
//! activité des modules. Accès : `site.stats`.

use crate::{
    context::Ctx,
    modules::{presences, recrues},
};
use anyhow::Result;
use twilight_model::id::{Id, marker::GuildMarker};

async fn count(ctx: &Ctx, sql: &str, guild_id: &str) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(sql).bind(guild_id).fetch_one(&ctx.db).await?;
    Ok(n)
}

/// Chiffre clé du serveur (page et API).
pub(crate) struct FigureData {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) hint: Option<String>,
}

/// Statistiques d'un serveur, membres en identifiants (page et API).
pub(crate) struct StatsData {
    pub(crate) figures: Vec<FigureData>,
    pub(crate) attendance: Vec<presences::service::LeaderboardEntry>,
    pub(crate) recruits: Vec<recrues::service::Entry>,
    pub(crate) tickets: Option<crate::modmail::TicketStats>,
}

pub(crate) async fn build(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<StatsData> {
    let gid = guild_id.to_string();

    let presences_total =
        count(ctx, r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL"#, &gid).await?;
    let presences_month = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL
           AND "eventDate" >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 days')"#,
        &gid,
    )
    .await?;
    let absences = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Absence" WHERE "guildId" = ? AND "endedAt" IS NULL
           AND "startsAt" <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
        &gid,
    )
    .await?;
    let transactions = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Transaction" WHERE "guildId" = ? AND "statut" NOT IN ('LIVRE_PAYE', 'ANNULEE', 'CLOTUREE')"#,
        &gid,
    )
    .await?;
    let vehicles = count(ctx, r#"SELECT COUNT(*) FROM "Vehicle" WHERE "guildId" = ?"#, &gid).await?;

    let leaderboard = presences::service::leaderboard(&ctx.db, &gid).await?;
    let average = if leaderboard.is_empty() {
        0
    } else {
        leaderboard.iter().map(|e| e.percent).sum::<i64>() / leaderboard.len() as i64
    };
    let session = recrues::service::current_session(&ctx.db, &gid).await?;
    let recruits = session.map(|s| s.entries).unwrap_or_default();

    let figure = |key, label, value: String, hint| FigureData { key, label, value, hint };
    let figures = vec![
        figure(
            "presences",
            "Présences clôturées",
            presences_total.to_string(),
            Some(format!("{presences_month} ces 30 derniers jours")),
        ),
        figure("attendance", "Taux de présence moyen", format!("{average} %"), None),
        figure("absences", "Absences en cours", absences.to_string(), None),
        figure("recruits", "Recrues en essai", recruits.len().to_string(), None),
        figure("transactions", "Transactions en cours", transactions.to_string(), None),
        figure("vehicles", "Véhicules enregistrés", vehicles.to_string(), None),
    ];

    let tickets = match &ctx.modmail {
        Some(modmail) if modmail.config.main_server_ids.contains(&guild_id) => Some(modmail.ticket_stats(ctx).await?),
        _ => None,
    };

    Ok(StatsData { figures, attendance: leaderboard, recruits, tickets })
}

#[cfg(test)]
mod tests {}
