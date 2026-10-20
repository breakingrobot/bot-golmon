//! Accueil d'un serveur : quelques chiffres utiles tout de suite, selon les
//! droits de la personne. Chaque tuile est `null` sans le droit correspondant
//! (ou quand le module n'est pas utilisé sur le serveur). Lecture seule.

use super::{ApiResult, MemberRef, Members, Session, iso_db, json, openapi};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit,
        coffres::{alerts::Thresholds, parser::item_label, service as coffres},
        permissions::Perm,
        presences::service as presences,
        recrues::service::{self as recrues, Entry as RecrueEntry},
    },
    util::time::DbTime,
};
use axum::extract::{Path, State};
use serde::Serialize;
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::ToSchema;

/// Objets sous leur seuil montrés sur la tuile.
const LOW_STOCK_SHOWN: usize = 3;
/// Noms des absents montrés sur la tuile.
const ABSENTS_SHOWN: usize = 8;

#[derive(Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LowStockItem {
    pub property_id: String,
    #[schema(required = true)]
    pub property_name: Option<String>,
    pub item: String,
    pub label: String,
    pub quantity: i64,
    pub min: i64,
}

#[derive(Debug, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewLowStock)]
pub struct LowStock {
    /// Nombre d'objets sous leur seuil (toutes propriétés).
    pub count: usize,
    /// Les plus en manque d'abord, 3 au plus.
    pub items: Vec<LowStockItem>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewObjective)]
pub struct ObjectiveTile {
    /// Avancement de la personne cette semaine ; `null` sans objectif configuré.
    #[schema(required = true)]
    pub progress: Option<openapi::ObjectiveProgress>,
    /// Membres l'ayant atteint (avec `cotisation.stats`, sinon `null`).
    #[schema(required = true)]
    pub reached: Option<usize>,
    /// Membres concernés cette semaine (hors exemptés ; avec `cotisation.stats`).
    #[schema(required = true)]
    pub tracked: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewNextPresence)]
pub struct NextPresence {
    pub id: String,
    pub title: String,
    pub date: String,
    #[schema(value_type = openapi::PresenceStatus)]
    pub my_status: String,
    pub responses: usize,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewPresence)]
pub struct PresenceTile {
    /// Présence ouverte la plus proche, `null` s'il n'y en a pas.
    #[schema(required = true)]
    pub next: Option<NextPresence>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewAbsences)]
pub struct AbsencesTile {
    pub count: usize,
    /// Absents aujourd'hui (8 au plus, fin la plus proche d'abord).
    pub members: Vec<MemberRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewRecrues)]
pub struct RecruesTile {
    /// Recrues de la semaine en cours.
    pub count: usize,
    /// Avis que la personne n'a pas encore donnés (`null` : elle ne donne pas d'avis).
    #[schema(required = true)]
    pub missing_votes: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewTransactions)]
pub struct TransactionsTile {
    /// Transactions en cours (ni livrées, ni annulées, ni clôturées).
    pub count: i64,
    /// Montant engagé dans ces transactions.
    pub amount: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewTickets)]
pub struct TicketsTile {
    pub open: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewJournalEntry)]
pub struct LastActivity {
    pub at: String,
    pub actor: MemberRef,
    pub label: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = OverviewJournal)]
pub struct JournalTile {
    /// Dernière entrée du journal, `null` s'il est vide.
    #[schema(required = true)]
    pub last: Option<LastActivity>,
}

#[derive(Debug, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuildOverview {
    /// `coffre.voir`, module coffre utilisé.
    #[schema(required = true)]
    pub low_stock: Option<LowStock>,
    /// `cotisation.ajouter`.
    #[schema(required = true)]
    pub objective: Option<ObjectiveTile>,
    /// `presence.stats`.
    #[schema(required = true)]
    pub presence: Option<PresenceTile>,
    /// `presence.stats`.
    #[schema(required = true)]
    pub absences: Option<AbsencesTile>,
    /// `recrue.suivi`.
    #[schema(required = true)]
    pub recrues: Option<RecruesTile>,
    /// `transaction.utiliser`.
    #[schema(required = true)]
    pub transactions: Option<TransactionsTile>,
    /// Staff des tickets, sur le serveur principal du module ticket.
    #[schema(required = true)]
    pub tickets: Option<TicketsTile>,
    /// `site.journal`.
    #[schema(required = true)]
    pub journal: Option<JournalTile>,
}

/// Objets sous leur seuil, les plus en manque (part du seuil atteinte) d'abord.
pub fn low_stock(lines: &[coffres::StockLine], thresholds: &Thresholds, names: &HashMap<String, String>) -> LowStock {
    let mut low: Vec<LowStockItem> = lines
        .iter()
        .filter_map(|line| {
            let min = thresholds.min_for(&line.property_id, &line.item)?;
            let quantity = line.quantity();
            (quantity < min).then(|| LowStockItem {
                property_id: line.property_id.clone(),
                property_name: names.get(&line.property_id).cloned(),
                item: line.item.clone(),
                label: item_label(&line.item),
                quantity,
                min,
            })
        })
        .collect();
    // quantité / seuil croissant, sans division : a.q × b.min < b.q × a.min.
    low.sort_by(|a, b| (a.quantity * b.min).cmp(&(b.quantity * a.min)).then_with(|| a.label.cmp(&b.label)));
    let count = low.len();
    low.truncate(LOW_STOCK_SHOWN);
    LowStock { count, items: low }
}

/// Recrues de la semaine sur lesquelles `voter` n'a pas encore donné d'avis (jamais sur soi).
pub fn missing_votes(entries: &[RecrueEntry], voter: &str) -> usize {
    entries.iter().filter(|e| e.user_id != voter && !e.votes.iter().any(|v| v.voter_id == voter)).count()
}

async fn coffres_used(ctx: &Ctx, guild: &str) -> anyhow::Result<bool> {
    let (used,): (bool,) = sqlx::query_as(
        r#"SELECT EXISTS (SELECT 1 FROM "CoffreChannel" WHERE "guildId" = ?1)
              OR EXISTS (SELECT 1 FROM "CoffreStock" WHERE "guildId" = ?1)"#,
    )
    .bind(guild)
    .fetch_one(&ctx.db)
    .await?;
    Ok(used)
}

async fn low_stock_tile(ctx: &Ctx, guild: &str) -> anyhow::Result<Option<LowStock>> {
    if !coffres_used(ctx, guild).await? {
        return Ok(None);
    }
    let lines = coffres::stock(&ctx.db, guild, None).await?;
    let thresholds = crate::modules::coffres::alerts::thresholds(&ctx.db, guild).await?;
    let names = super::coffres::property_names(&ctx.db, guild).await?;
    Ok(Some(low_stock(&lines, &thresholds, &names)))
}

async fn objective_tile(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    user: &str,
    stats: bool,
) -> anyhow::Result<ObjectiveTile> {
    let progress = super::reglages::member_week(ctx, guild_id, user).await?;
    let (reached, tracked) = if stats && progress.is_some() {
        let week = super::reglages::week_progress(ctx, guild_id).await?;
        let concerned: Vec<_> = week.iter().filter(|p| !p.exempt).collect();
        (Some(concerned.iter().filter(|p| p.reached).count()), Some(concerned.len()))
    } else {
        (None, None)
    };
    Ok(ObjectiveTile { progress, reached, tracked })
}

async fn presence_tile(ctx: &Ctx, guild: &str, user: &str) -> anyhow::Result<PresenceTile> {
    let Some(presence) = presences::current_presence(&ctx.db, guild).await? else {
        return Ok(PresenceTile { next: None });
    };
    let responses = presences::responses(&ctx.db, &presence.id).await?;
    let my_status =
        responses.iter().find(|r| r.user_id == user).map_or_else(|| "NON_REPONDU".to_owned(), |r| r.status.clone());
    Ok(PresenceTile {
        next: Some(NextPresence {
            id: presence.id,
            title: presence.titre,
            date: iso_db(presence.event_date),
            my_status,
            responses: responses.len(),
        }),
    })
}

async fn absences_tile(ctx: &Ctx, guild_id: Id<GuildMarker>) -> anyhow::Result<AbsencesTile> {
    let now = DbTime::now();
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "userId" FROM "Absence"
           WHERE "guildId" = ? AND "endedAt" IS NULL AND "startsAt" <= ? AND "endsAt" > ?
           GROUP BY "userId" ORDER BY MIN("endsAt") ASC"#,
    )
    .bind(guild_id.to_string())
    .bind(now)
    .bind(now)
    .fetch_all(&ctx.db)
    .await?;
    let members = Members::new(ctx, guild_id);
    Ok(AbsencesTile {
        count: rows.len(),
        members: rows.iter().take(ABSENTS_SHOWN).map(|(user,)| members.get(user)).collect(),
    })
}

async fn transactions_tile(ctx: &Ctx, guild: &str) -> anyhow::Result<TransactionsTile> {
    let (count, amount): (i64, f64) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM("prix"), 0.0) FROM "Transaction"
           WHERE "guildId" = ? AND "statut" NOT IN ('LIVRE_PAYE', 'ANNULEE', 'CLOTUREE')"#,
    )
    .bind(guild)
    .fetch_one(&ctx.db)
    .await?;
    Ok(TransactionsTile { count, amount })
}

async fn journal_tile(ctx: &Ctx, guild_id: Id<GuildMarker>) -> anyhow::Result<JournalTile> {
    let (rows, _) = audit::recent(&ctx.db, &guild_id.to_string(), None, 1, 0).await?;
    let members = Members::new(ctx, guild_id);
    Ok(JournalTile {
        last: rows.into_iter().next().map(|row| LastActivity {
            at: iso_db(row.created_at),
            actor: members.get_or(&row.actor_id, Some(&row.actor_name)),
            label: audit::Action::from_code(&row.action).map_or(row.action.clone(), |a| a.label().to_owned()),
        }),
    })
}

/// Accueil du serveur : une tuile par module ouvert à la personne.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/overview", tag = "stats", operation_id = "getGuildOverview",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Tuiles de l'accueil", body = GuildOverview),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Pas membre du serveur", body = super::ErrorBody),
    )
)]
pub async fn overview(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let gid = guild_id.to_string();
    let me = session.viewer.id.to_string();
    let can = |perm| session.can(guild_id, perm);
    let mut overview = GuildOverview::default();

    if can(Perm::CoffreVoir) {
        overview.low_stock = low_stock_tile(&ctx, &gid).await?;
    }
    if can(Perm::CotisationAjouter) {
        overview.objective = Some(objective_tile(&ctx, guild_id, &me, can(Perm::CotisationStats)).await?);
    }
    if can(Perm::PresenceStats) {
        overview.presence = Some(presence_tile(&ctx, &gid, &me).await?);
        overview.absences = Some(absences_tile(&ctx, guild_id).await?);
    }
    if can(Perm::RecrueSuivi) {
        let entries = recrues::current_session(&ctx.db, &gid).await?.map(|s| s.entries).unwrap_or_default();
        overview.recrues = Some(RecruesTile {
            count: entries.len(),
            missing_votes: can(Perm::RecrueAvis).then(|| missing_votes(&entries, &me)),
        });
    }
    if can(Perm::TransactionUtiliser) {
        overview.transactions = Some(transactions_tile(&ctx, &gid).await?);
    }
    if session.access.tickets
        && let Some(modmail) = &ctx.modmail
        && modmail.config.main_server_ids.contains(&guild_id)
    {
        overview.tickets = Some(TicketsTile { open: modmail.open_tickets().await? });
    }
    if can(Perm::Journal) {
        overview.journal = Some(journal_tile(&ctx, guild_id).await?);
    }
    Ok(json(&overview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{coffres::alerts::Threshold, recrues::service::VoteRow};

    fn line(property: &str, item: &str, quantity: i64) -> coffres::StockLine {
        coffres::StockLine {
            property_id: property.into(),
            item: item.into(),
            deposited: quantity,
            withdrawn: 0,
            adjusted: 0,
        }
    }

    #[test]
    fn low_stock_lists_the_most_missing_first() {
        let thresholds = Thresholds::new(&[
            Threshold { property_id: None, item: "kit".into(), min: 10 },
            Threshold { property_id: Some("2".into()), item: "pain".into(), min: 100 },
            Threshold { property_id: None, item: "eau".into(), min: 4 },
        ]);
        let lines = vec![
            line("1", "kit", 9),   // 90 %
            line("2", "kit", 2),   // 20 %
            line("1", "pain", 0),  // pas de seuil sur la propriété 1
            line("2", "pain", 50), // 50 %
            line("1", "eau", 0),   // 0 %
            line("1", "pomme", 0), // pas de seuil
        ];
        let names: HashMap<String, String> = [("2".to_owned(), "Planque".to_owned())].into();
        let low = low_stock(&lines, &thresholds, &names);
        assert_eq!(low.count, 4);
        let shown: Vec<(&str, &str)> = low.items.iter().map(|i| (i.property_id.as_str(), i.item.as_str())).collect();
        assert_eq!(shown, [("1", "eau"), ("2", "kit"), ("2", "pain")]);
        assert_eq!(low.items[1].property_name.as_deref(), Some("Planque"));
        assert_eq!((low.items[1].quantity, low.items[1].min), (2, 10));
    }

    #[test]
    fn missing_votes_skip_own_entry_and_given_votes() {
        let entry = |user: &str, voters: &[&str]| RecrueEntry {
            id: user.into(),
            session_id: "s".into(),
            user_id: user.into(),
            veto_by_id: None,
            veto_motif: None,
            votes: voters
                .iter()
                .map(|v| VoteRow {
                    entry_id: user.into(),
                    voter_id: (*v).into(),
                    choix: "POUR".into(),
                    motif: String::new(),
                })
                .collect(),
        };
        let entries = vec![entry("a", &["moi"]), entry("b", &["autre"]), entry("c", &[]), entry("moi", &[])];
        assert_eq!(missing_votes(&entries, "moi"), 2);
        assert_eq!(missing_votes(&[], "moi"), 0);
    }

    #[test]
    fn hidden_tiles_are_null() {
        let value = serde_json::to_value(GuildOverview::default()).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 8);
        assert!(object.values().all(serde_json::Value::is_null));
    }
}
