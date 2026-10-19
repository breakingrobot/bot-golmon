//! Historique des transactions (`transaction.historique`), mêmes filtres et
//! mêmes totaux que la page du site.

use super::{ApiQuery, ApiResult, MemberRef, Members, Page, Session, clamp_page, iso_db, json, page_number};
use crate::{
    context::Context,
    modules::{permissions::Perm, transactions::model::Status},
    web::transactions::{self as page, Filters, HistoryQuery, HistoryRow, MONTHS_SHOWN, PAGE_SIZE, Summary},
};
use axum::extract::{Path, State};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    pub created_at: String,
    pub groupe: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub status_label: &'static str,
    pub prix: f64,
    pub creator: MemberRef,
    pub items: Vec<Item>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub nom: String,
    pub quantite: i64,
    pub prix_unitaire: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub count: usize,
    pub achats: f64,
    pub ventes: f64,
    pub solde: f64,
    pub in_progress: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Month {
    pub month: String,
    pub achats: f64,
    pub ventes: f64,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChoice {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionsResponse {
    pub totals: Totals,
    pub months: Vec<Month>,
    pub statuses: Vec<StatusChoice>,
    pub list: Page<Transaction>,
}

fn transaction(members: &Members, row: HistoryRow) -> Transaction {
    let status = Status::parse(&row.statut);
    Transaction {
        id: row.id,
        created_at: iso_db(row.created_at),
        groupe: row.groupe,
        kind: row.kind,
        status_label: status.map_or("Inconnu", Status::label),
        status: row.statut,
        prix: row.prix,
        creator: members.get(&row.created_by_id),
        items: row
            .items
            .into_iter()
            .map(|item| Item { nom: item.nom, quantite: item.quantite, prix_unitaire: item.prix_unitaire })
            .collect(),
    }
}

pub fn summary_json(summary: &Summary) -> (Totals, Vec<Month>) {
    let t = &summary.totals;
    let totals = Totals {
        count: t.count,
        achats: t.achats,
        ventes: t.ventes,
        solde: t.solde(),
        in_progress: summary.in_progress,
    };
    let months = summary
        .months
        .iter()
        .take(MONTHS_SHOWN)
        .map(|((year, month), t)| Month {
            month: format!("{year:04}-{month:02}"),
            achats: t.achats,
            ventes: t.ventes,
            count: t.count,
        })
        .collect();
    (totals, months)
}

pub async fn list(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<HistoryQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionHistorique)?;
    let gid = guild_id.to_string();
    let filters = Filters::from_query(&query);
    let amounts = page::amounts(&ctx.db, &gid, &filters).await?;
    let (page_number, offset) = clamp_page(page_number(query.page.as_deref()), amounts.len() as i64, PAGE_SIZE);
    let rows = page::rows(&ctx.db, &gid, &filters, PAGE_SIZE, offset).await?;
    let summary = page::summarize(&amounts);
    let (totals, months) = summary_json(&summary);
    let members = Members::new(&ctx, guild_id);
    Ok(json(&TransactionsResponse {
        totals,
        months,
        statuses: Status::ALL.iter().map(|s| StatusChoice { value: s.as_str(), label: s.label() }).collect(),
        list: Page {
            items: rows.into_iter().map(|row| transaction(&members, row)).collect(),
            total: amounts.len() as i64,
            page: page_number,
            page_size: PAGE_SIZE,
        },
    }))
}

pub async fn export(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<HistoryQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::TransactionHistorique)?;
    Ok(page::csv_response(&ctx, guild_id, &query).await)
}
