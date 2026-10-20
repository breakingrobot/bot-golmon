//! Tableau de bord permanent des cotisations (Components V2) : classement de la
//! semaine et avancement de l'objectif. Mis à jour à chaque cotisation ajoutée
//! ou supprimée, et au changement de semaine (cron). Les mentions sont
//! affichées, jamais notifiées.

use super::{
    money,
    objective::{self, Objective, ProgressRow},
    service::{self, Filter, Totals},
};
use crate::{
    context::Ctx,
    discord::{
        components::{container, separator, text_display, text_displays},
        messages::{self, Outgoing, parse_id},
    },
    util::{
        format::{ListBlock, V2_TEXT_LIMIT, and_others, pack_blocks},
        time::{DbTime, week_start},
    },
};
use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::GuildMarker},
};

const ACCENT_COLOR: u32 = 0x2ecc71;
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 300;
/// Membres du classement.
const LEADERBOARD_SIZE: usize = 10;

/// Salon et message du tableau de bord.
pub async fn get_dashboard(db: &SqlitePool, guild_id: &str) -> Result<(Option<String>, Option<String>)> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT "dashboardChannelId", "dashboardMessageId" FROM "CotisationConfig" WHERE "guildId" = ?"#,
    )
    .bind(guild_id)
    .fetch_optional(db)
    .await?;
    Ok(row.unwrap_or_default())
}

/// Change le salon du tableau (`None` : plus de tableau). Le message connu est
/// oublié : l'appelant supprime l'ancien, puis le tableau est republié.
pub async fn set_dashboard_channel(db: &SqlitePool, guild_id: &str, channel_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "CotisationConfig" ("guildId", "dashboardChannelId", "dashboardMessageId", "updatedAt") VALUES (?, ?, NULL, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "dashboardChannelId" = excluded."dashboardChannelId",
             "dashboardMessageId" = NULL, "dashboardWeek" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Serveurs dont le tableau affiche encore une semaine passée.
pub async fn stale_dashboards(db: &SqlitePool, week: DbTime) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "guildId" FROM "CotisationConfig"
           WHERE "dashboardChannelId" IS NOT NULL AND ("dashboardWeek" IS NULL OR "dashboardWeek" < ?)"#,
    )
    .bind(week)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Contenu du tableau pour une semaine.
pub struct Board<'a> {
    pub week: DbTime,
    pub totals: &'a Totals,
    /// Membres de la semaine, du plus gros contributeur au plus petit.
    pub members: &'a [(String, Totals)],
    pub objective: Option<&'a Objective>,
    pub progress: &'a [ProgressRow],
}

/// Éléments séparés par des virgules dans la limite de `budget` octets, la
/// suite résumée par « … et N autres ».
pub fn inline_list(items: &[String], budget: usize) -> String {
    let mut text = String::new();
    for (index, item) in items.iter().enumerate() {
        let rest = items.len() - index - 1;
        let reserve = if rest > 0 { and_others(rest).len() + 2 } else { 0 };
        if !text.is_empty() && text.len() + 2 + item.len() + reserve > budget {
            text.push_str(&format!(" {}", and_others(items.len() - index)));
            return text;
        }
        if !text.is_empty() {
            text.push_str(", ");
        }
        text.push_str(item);
    }
    text
}

pub fn build(board: &Board) -> Vec<Component> {
    let footer = format!("-# Déclarer : `/cotisation ajouter` · mis à jour <t:{}:R>", DbTime::now().unix());
    let header = format!(
        "## 💰 Cotisations de la semaine\nSemaine {} · **{}** ({} propre · {} sale) · {} cotisation{}",
        objective::week_label(board.week),
        money(board.totals.total()),
        money(board.totals.propre),
        money(board.totals.sale),
        board.totals.count,
        if board.totals.count > 1 { "s" } else { "" }
    );

    let ranking: Vec<String> = board
        .members
        .iter()
        .take(LEADERBOARD_SIZE)
        .enumerate()
        .map(|(rank, (user, t))| {
            let medal = match rank {
                0 => "🥇".to_owned(),
                1 => "🥈".to_owned(),
                2 => "🥉".to_owned(),
                n => format!("{}.", n + 1),
            };
            format!(
                "{medal} <@{user}> — **{}** ({} propre · {} sale)",
                money(t.total()),
                money(t.propre),
                money(t.sale)
            )
        })
        .collect();
    let mut blocks = vec![ListBlock {
        header: "### 🏆 Classement".to_owned(),
        lines: ranking,
        empty: "*Aucune cotisation.*".into(),
    }];

    if let Some(objective) = board.objective {
        let tracked: Vec<&ProgressRow> = board.progress.iter().filter(|r| !r.exempt).collect();
        let reached = tracked.iter().filter(|r| r.reached).count();
        let below: Vec<String> = tracked
            .iter()
            .filter(|r| !r.reached)
            .map(|r| format!("- <@{}> — {} / {}", r.user_id, money(r.amount), money(r.target)))
            .collect();
        blocks.push(ListBlock {
            header: format!(
                "### 🎯 Objectif : {}\n✅ **{reached}/{}** membre{} l'{} atteint · ⏳ en dessous :",
                objective.describe(),
                tracked.len(),
                if tracked.len() > 1 { "s" } else { "" },
                if reached > 1 { "ont" } else { "a" }
            ),
            lines: below,
            empty: if tracked.is_empty() {
                "*Aucun membre suivi (rôle membre : `/config permission role_membre:@…`).*".into()
            } else {
                "*Tout le monde a atteint l'objectif. 🎉*".into()
            },
        });
        let exempt: Vec<String> =
            board.progress.iter().filter(|r| r.exempt).map(|r| format!("<@{}>", r.user_id)).collect();
        if !exempt.is_empty() {
            blocks.push(ListBlock {
                header: format!("### 🌙 Non concernés cette semaine ({})", exempt.len()),
                lines: vec![inline_list(&exempt, 600)],
                empty: String::new(),
            });
        }
    }

    let budget = TEXT_BUDGET.saturating_sub(header.len() + footer.len());
    let mut components = vec![text_display(header)];
    for block in pack_blocks(&blocks, budget) {
        components.push(separator());
        components.extend(text_displays(&block));
    }
    components.push(separator());
    components.push(text_display(footer));
    vec![container(Some(ACCENT_COLOR), components)]
}

/// Met à jour (ou publie) le tableau de la semaine en cours, s'il est configuré.
pub async fn sync_dashboard(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let (channel, known) = get_dashboard(&ctx.db, &guild).await?;
    let Some(channel_id) = channel.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let week = week_start(Utc::now());
    let filter = Filter { since: Some(week), until: Some(objective::week_end(week)), ..Default::default() };
    let totals = service::totals(&ctx.db, &guild, &filter).await?;
    let members = service::by_member(&ctx.db, &guild, &filter).await?;
    let objective = objective::get_objective(&ctx.db, &guild).await?;
    let progress = match &objective {
        Some(objective) => {
            let tracked = objective::tracked_members(ctx, guild_id).await?;
            objective::progress_for(&ctx.db, &guild, week, objective, &tracked).await?
        }
        None => Vec::new(),
    };
    let board = Board { week, totals: &totals, members: &members, objective: objective.as_ref(), progress: &progress };
    let message = Outgoing::components_v2(build(&board));
    match messages::upsert(ctx, channel_id, known.as_deref().and_then(parse_id), &message).await {
        Ok(new_id) => {
            sqlx::query(
                r#"UPDATE "CotisationConfig" SET "dashboardMessageId" = COALESCE(?, "dashboardMessageId"), "dashboardWeek" = ?
                   WHERE "guildId" = ?"#,
            )
            .bind(new_id.map(|id| id.to_string()))
            .bind(week)
            .bind(&guild)
            .execute(&ctx.db)
            .await?;
        }
        Err(error) => tracing::warn!(%guild_id, ?error, "tableau des cotisations inaccessible"),
    }
    Ok(())
}

/// Met à jour le tableau sans faire échouer l'action qui l'a déclenchée.
pub async fn refresh(ctx: &Ctx, guild_id: Id<GuildMarker>) {
    ctx.events.publish(guild_id, crate::events::Topic::Cotisations);
    if let Err(error) = sync_dashboard(ctx, guild_id).await {
        tracing::warn!(%guild_id, ?error, "mise à jour du tableau des cotisations impossible");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        modules::cotisations::objective::Counts,
        util::format::V2_COMPONENT_LIMIT,
    };

    #[test]
    fn board_lists_ranking_and_objective_within_limits() {
        let week = week_start(Utc::now());
        let members: Vec<(String, Totals)> = (0..30)
            .map(|i| {
                (format!("{}", 100_000_000_000_000_000u64 + i), Totals { count: 1, propre: 1000 - i as i64, sale: 0 })
            })
            .collect();
        let totals = Totals { count: 30, propre: 29_565, sale: 0 };
        let objective = Objective { amount: 999, counts: Counts::Total, reminder: true };
        let mut progress: Vec<ProgressRow> = (0..120)
            .map(|i| ProgressRow {
                user_id: format!("{}", 200_000_000_000_000_000u64 + i),
                amount: i as i64,
                target: 999,
                reached: false,
                exempt: i % 7 == 0,
            })
            .collect();
        progress.push(ProgressRow { user_id: "1".into(), amount: 1000, target: 999, reached: true, exempt: false });
        let board =
            Board { week, totals: &totals, members: &members, objective: Some(&objective), progress: &progress };
        let components = build(&board);
        let (text, count) = v2_usage(&components);
        assert!(text <= V2_TEXT_LIMIT, "{text}");
        assert!(count <= V2_COMPONENT_LIMIT);
        let rendered = v2_text(&components);
        assert!(rendered.contains("🥇 <@100000000000000000>"));
        assert!(rendered.contains("**1/103** membres l'a atteint"));
        assert!(rendered.contains("Non concernés cette semaine (18)"));
        assert!(!rendered.contains("11. <@"), "classement limité à 10");

        let empty = Board { week, totals: &Totals::default(), members: &[], objective: None, progress: &[] };
        let rendered = v2_text(&build(&empty));
        assert!(rendered.contains("Aucune cotisation") && !rendered.contains("Objectif"));
    }
}
