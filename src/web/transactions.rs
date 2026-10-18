//! Historique des transactions d'un serveur : filtres, totaux par mois, détail
//! des objets et export CSV. Même accès que les statistiques (« Gérer le serveur »).

use super::{
    MaybeViewer, Viewer,
    auth::url_encode,
    pages::{self, Layout},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::{
        like_contains,
        transactions::model::{Status, TYPE_CHOICES, TransactionItem, type_label},
    },
    util::{
        format::{format_price, plural},
        time::{DbTime, TIMEZONE, format_date_label, paris_day_start, parse_iso_day},
    },
};
use anyhow::Result;
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};
use std::collections::{BTreeMap, HashMap};
use twilight_model::id::{Id, marker::GuildMarker};

const PAGE_SIZE: i64 = 25;
const CSV_LIMIT: i64 = 10_000;
const MONTHS_SHOWN: usize = 12;

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

#[derive(Debug, Default, Deserialize)]
pub struct HistoryQuery {
    groupe: Option<String>,
    statut: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    du: Option<String>,
    au: Option<String>,
    /// En texte : une valeur invalide ramène à la première page plutôt qu'à une erreur 400.
    page: Option<String>,
}

/// Filtres validés : une valeur inconnue est ignorée, jamais injectée telle quelle.
#[derive(Clone, Debug, Default, PartialEq)]
struct Filters {
    groupe: String,
    statut: Option<Status>,
    kind: Option<&'static str>,
    du: Option<NaiveDate>,
    au: Option<NaiveDate>,
}

impl Filters {
    fn from_query(query: &HistoryQuery) -> Self {
        let text = |value: &Option<String>| value.as_deref().unwrap_or_default().trim().to_owned();
        Self {
            groupe: text(&query.groupe),
            statut: query.statut.as_deref().and_then(Status::parse),
            kind: query
                .kind
                .as_deref()
                .and_then(|kind| TYPE_CHOICES.iter().map(|(_, value)| *value).find(|value| *value == kind)),
            du: query.du.as_deref().and_then(parse_iso_day),
            au: query.au.as_deref().and_then(parse_iso_day),
        }
    }

    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Paramètres d'URL des filtres actifs (sans `?`), pour la pagination et l'export.
    fn query_string(&self) -> String {
        let mut parts = Vec::new();
        if !self.groupe.is_empty() {
            parts.push(format!("groupe={}", url_encode(&self.groupe)));
        }
        if let Some(statut) = self.statut {
            parts.push(format!("statut={}", statut.as_str()));
        }
        if let Some(kind) = self.kind {
            parts.push(format!("type={kind}"));
        }
        if let Some(du) = self.du {
            parts.push(format!("du={du}"));
        }
        if let Some(au) = self.au {
            parts.push(format!("au={au}"));
        }
        parts.join("&")
    }

    /// Clause `WHERE` et valeurs liées. Les bornes de dates sont des jours
    /// entiers en heure de Paris : `au` est inclus (on s'arrête au minuit suivant).
    fn where_clause(&self, guild_id: &str) -> (String, Vec<String>) {
        let mut sql = String::from(r#""guildId" = ?"#);
        let mut binds = vec![guild_id.to_owned()];
        if !self.groupe.is_empty() {
            sql.push_str(r#" AND "groupe" LIKE ? ESCAPE '\'"#);
            binds.push(like_contains(&self.groupe));
        }
        if let Some(statut) = self.statut {
            sql.push_str(r#" AND "statut" = ?"#);
            binds.push(statut.as_str().to_owned());
        }
        if let Some(kind) = self.kind {
            sql.push_str(r#" AND "type" = ?"#);
            binds.push(kind.to_owned());
        }
        if let Some(du) = self.du {
            sql.push_str(r#" AND "createdAt" >= ?"#);
            binds.push(paris_day_start(du).to_db_string());
        }
        if let Some(next) = self.au.and_then(|au| au.succ_opt()) {
            sql.push_str(r#" AND "createdAt" < ?"#);
            binds.push(paris_day_start(next).to_db_string());
        }
        (sql, binds)
    }
}

/// Le strict nécessaire pour les totaux, sur tout l'ensemble filtré.
#[derive(Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
struct Amount {
    #[sqlx(rename = "type")]
    kind: String,
    prix: f64,
    statut: String,
    created_at: DbTime,
}

#[derive(Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
struct HistoryRow {
    id: String,
    groupe: String,
    #[sqlx(rename = "type")]
    kind: String,
    prix: f64,
    statut: String,
    created_by_id: String,
    created_at: DbTime,
    #[sqlx(skip)]
    items: Vec<TransactionItem>,
}

async fn amounts(db: &SqlitePool, guild_id: &str, filters: &Filters) -> Result<Vec<Amount>> {
    let (clause, binds) = filters.where_clause(guild_id);
    let sql = format!(r#"SELECT "type", "prix", "statut", "createdAt" FROM "Transaction" WHERE {clause}"#);
    let mut query = sqlx::query_as::<_, Amount>(&sql);
    for value in binds {
        query = query.bind(value);
    }
    Ok(query.fetch_all(db).await?)
}

/// Transactions filtrées, les plus récentes d'abord, avec leurs objets.
async fn rows(db: &SqlitePool, guild_id: &str, filters: &Filters, limit: i64, offset: i64) -> Result<Vec<HistoryRow>> {
    let (clause, binds) = filters.where_clause(guild_id);
    let selection = format!(
        r#"SELECT "id", "groupe", "type", "prix", "statut", "createdById", "createdAt" FROM "Transaction"
           WHERE {clause} ORDER BY "createdAt" DESC, "id" DESC LIMIT ? OFFSET ?"#
    );
    let mut query = sqlx::query_as::<_, HistoryRow>(&selection);
    for value in &binds {
        query = query.bind(value);
    }
    let mut rows = query.bind(limit).bind(offset).fetch_all(db).await?;
    if rows.is_empty() {
        return Ok(rows);
    }

    // Même sélection en sous-requête : pas de liste de milliers d'identifiants à lier.
    let items_sql = format!(
        r#"SELECT "transactionId", "nom", "quantite", "prixUnitaire" FROM "TransactionItem"
           WHERE "transactionId" IN (SELECT "id" FROM "Transaction" WHERE {clause} ORDER BY "createdAt" DESC, "id" DESC LIMIT ? OFFSET ?)
           ORDER BY "createdAt" ASC, "id" ASC"#
    );
    let mut query = sqlx::query_as::<_, TransactionItem>(&items_sql);
    for value in &binds {
        query = query.bind(value);
    }
    let mut by_transaction: HashMap<String, Vec<TransactionItem>> = HashMap::new();
    for item in query.bind(limit).bind(offset).fetch_all(db).await? {
        by_transaction.entry(item.transaction_id.clone()).or_default().push(item);
    }
    for row in &mut rows {
        row.items = by_transaction.remove(&row.id).unwrap_or_default();
    }
    Ok(rows)
}

#[derive(Debug, Default, PartialEq)]
struct Totals {
    count: usize,
    achats: f64,
    ventes: f64,
}

impl Totals {
    /// Les transactions annulées comptent dans le nombre, pas dans les montants.
    fn add(&mut self, amount: &Amount) {
        self.count += 1;
        if amount.statut == Status::Annulee.as_str() {
            return;
        }
        if amount.kind == "ACHAT" {
            self.achats += amount.prix;
        } else {
            self.ventes += amount.prix;
        }
    }

    fn solde(&self) -> f64 {
        self.ventes - self.achats
    }
}

#[derive(Debug, Default, PartialEq)]
struct Summary {
    totals: Totals,
    in_progress: usize,
    /// Mois (année, mois) en heure de Paris, du plus récent au plus ancien.
    months: Vec<((i32, u32), Totals)>,
}

fn summarize(amounts: &[Amount]) -> Summary {
    let mut summary = Summary::default();
    let mut months: BTreeMap<(i32, u32), Totals> = BTreeMap::new();
    for amount in amounts {
        summary.totals.add(amount);
        if Status::parse(&amount.statut).is_some_and(|s| Status::ACTIVE.contains(&s)) {
            summary.in_progress += 1;
        }
        let local = amount.created_at.0.with_timezone(&TIMEZONE);
        months.entry((local.year(), local.month())).or_default().add(amount);
    }
    summary.months = months.into_iter().rev().collect();
    summary
}

fn month_label((year, month): (i32, u32)) -> String {
    format!("{} {year}", MONTHS[(month as usize).saturating_sub(1) % 12])
}

/// Montant affiché : `12 500 $`, séparateurs comme le tableau de bord Discord.
fn money(value: f64) -> String {
    format!("{}\u{202f}$", format_price(value))
}

fn solde_tone(value: f64) -> &'static str {
    if value < 0.0 { "text-danger" } else { "text-staff" }
}

fn status_classes(status: Status) -> &'static str {
    match status {
        Status::Preparation => "bg-system-soft text-system",
        Status::EnAttenteRdv => "bg-user-soft text-user",
        Status::AvanceAttentePaiement => "bg-internal-soft text-internal",
        Status::LivrePaye => "bg-staff-soft text-staff",
        Status::Annulee => "bg-danger-soft text-danger",
        Status::Cloturee => "bg-surface-sunken text-ink-subtle",
    }
}

fn type_classes(kind: &str) -> &'static str {
    if kind == "ACHAT" { "bg-accent-soft text-accent" } else { "bg-staff-soft text-staff" }
}

struct Figure {
    label: &'static str,
    value: String,
    hint: Option<String>,
    tone: &'static str,
}

struct MonthView {
    label: String,
    achats: String,
    ventes: String,
    solde: String,
    solde_tone: &'static str,
    count: usize,
}

struct ItemView {
    nom: String,
    quantite: i64,
    prix_unitaire: Option<String>,
}

struct RowView {
    date: String,
    groupe: String,
    type_label: &'static str,
    type_classes: &'static str,
    status_label: &'static str,
    status_classes: &'static str,
    prix: String,
    items: Vec<ItemView>,
    creator: String,
}

struct Choice {
    label: &'static str,
    value: &'static str,
    active: bool,
}

#[derive(Template)]
#[template(path = "transactions.html")]
struct TransactionsPage {
    layout: Layout,
    guild_id: String,
    guild_name: String,
    groupe: String,
    du: String,
    au: String,
    statuts: Vec<Choice>,
    types: Vec<Choice>,
    filtered: bool,
    figures: Vec<Figure>,
    months: Vec<MonthView>,
    months_hidden: usize,
    rows: Vec<RowView>,
    total: usize,
    page: i64,
    pages: i64,
    previous_href: Option<String>,
    next_href: Option<String>,
    csv_href: String,
}

impl TransactionsPage {
    fn plural(&self, count: usize) -> &'static str {
        plural(count)
    }
}

fn figures(summary: &Summary) -> Vec<Figure> {
    let totals = &summary.totals;
    let solde = totals.solde();
    vec![
        Figure { label: "Transactions", value: totals.count.to_string(), hint: None, tone: "text-ink" },
        Figure {
            label: "Total achats",
            value: money(totals.achats),
            hint: Some("hors annulées".into()),
            tone: "text-ink",
        },
        Figure {
            label: "Total ventes",
            value: money(totals.ventes),
            hint: Some("hors annulées".into()),
            tone: "text-ink",
        },
        Figure { label: "Solde", value: money(solde), hint: Some("ventes − achats".into()), tone: solde_tone(solde) },
        Figure { label: "En cours", value: summary.in_progress.to_string(), hint: None, tone: "text-ink" },
    ]
}

fn month_views(summary: &Summary) -> Vec<MonthView> {
    summary
        .months
        .iter()
        .take(MONTHS_SHOWN)
        .map(|(month, totals)| MonthView {
            label: month_label(*month),
            achats: money(totals.achats),
            ventes: money(totals.ventes),
            solde: money(totals.solde()),
            solde_tone: solde_tone(totals.solde()),
            count: totals.count,
        })
        .collect()
}

fn row_view(row: HistoryRow, creator: String) -> RowView {
    let status = Status::parse(&row.statut).unwrap_or(Status::Preparation);
    RowView {
        date: format_date_label(row.created_at),
        groupe: row.groupe,
        type_label: type_label(&row.kind),
        type_classes: type_classes(&row.kind),
        status_label: status.label(),
        status_classes: status_classes(status),
        prix: money(row.prix),
        items: row
            .items
            .into_iter()
            .map(|item| ItemView {
                nom: item.nom,
                quantite: item.quantite,
                prix_unitaire: item.prix_unitaire.map(money),
            })
            .collect(),
        creator,
    }
}

fn choices(filters: &Filters) -> (Vec<Choice>, Vec<Choice>) {
    let statuts = std::iter::once(Choice { label: "Tous", value: "", active: filters.statut.is_none() })
        .chain(Status::ALL.into_iter().map(|status| Choice {
            label: status.label(),
            value: status.as_str(),
            active: filters.statut == Some(status),
        }))
        .collect();
    let types = std::iter::once(Choice { label: "Tous", value: "", active: filters.kind.is_none() })
        .chain(TYPE_CHOICES.into_iter().map(|(label, value)| Choice {
            label,
            value,
            active: filters.kind == Some(value),
        }))
        .collect();
    (statuts, types)
}

/// Vérifie la connexion et le droit de voir le serveur ; renvoie son nom.
async fn authorize(
    ctx: &Ctx,
    viewer: MaybeViewer,
    uri: &Uri,
    guild_id: u64,
) -> Result<(Viewer, super::Access, Id<GuildMarker>, String), Box<Response>> {
    let path = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let (viewer, access) = pages::require_viewer(ctx, viewer, path).await?;
    let Some(guild_id) = Id::<GuildMarker>::new_checked(guild_id) else {
        return Err(Box::new(pages::not_found().await));
    };
    if !access.can_view_guild(guild_id) {
        return Err(Box::new(pages::forbidden(ctx, &viewer, &access)));
    }
    let name = access.guilds.iter().find(|(id, _)| *id == guild_id).map(|(_, n)| n.clone()).unwrap_or_default();
    Ok((viewer, access, guild_id, name))
}

fn creator_name(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    user_id.parse().ok().and_then(|id| members::display_name(ctx, guild_id, id)).unwrap_or_else(|| user_id.to_owned())
}

pub async fn history(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let (viewer, access, guild_id, guild_name) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let filters = Filters::from_query(&query);
    let gid = guild_id.to_string();
    let page = query.page.as_deref().and_then(|p| p.parse::<i64>().ok()).unwrap_or(1).max(1);

    let found = async {
        let amounts = amounts(&ctx.db, &gid, &filters).await?;
        let pages_count = ((amounts.len() as i64 + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
        let page = page.min(pages_count);
        let rows = rows(&ctx.db, &gid, &filters, PAGE_SIZE, (page - 1) * PAGE_SIZE).await?;
        anyhow::Ok((summarize(&amounts), rows, page, pages_count))
    }
    .await;
    let (summary, rows, page, pages_count) = match found {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(?error, %guild_id, "historique des transactions");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "L'historique est indisponible.");
        }
    };

    let base = format!("/stats/{gid}/transactions");
    let filter_query = filters.query_string();
    let href = |page: i64| {
        if filter_query.is_empty() {
            format!("{base}?page={page}")
        } else {
            format!("{base}?{filter_query}&page={page}")
        }
    };
    let csv_href = if filter_query.is_empty() { format!("{base}.csv") } else { format!("{base}.csv?{filter_query}") };
    let (statuts, types) = choices(&filters);
    let rows = rows
        .into_iter()
        .map(|row| {
            let creator = creator_name(&ctx, guild_id, &row.created_by_id);
            row_view(row, creator)
        })
        .collect();

    pages::render(&TransactionsPage {
        layout: Layout::new(&ctx, format!("Transactions · {guild_name}"), Some(&viewer), &access, "/stats"),
        guild_id: gid,
        guild_name,
        groupe: filters.groupe.clone(),
        du: filters.du.map(|d| d.to_string()).unwrap_or_default(),
        au: filters.au.map(|d| d.to_string()).unwrap_or_default(),
        statuts,
        types,
        filtered: !filters.is_empty(),
        figures: figures(&summary),
        months: month_views(&summary),
        months_hidden: summary.months.len().saturating_sub(MONTHS_SHOWN),
        rows,
        total: summary.totals.count,
        page,
        pages: pages_count,
        previous_href: (page > 1).then(|| href(page - 1)),
        next_href: (page < pages_count).then(|| href(page + 1)),
        csv_href,
    })
}

pub async fn export(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let (_, _, guild_id, _) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let filters = Filters::from_query(&query);
    let gid = guild_id.to_string();
    let rows = match rows(&ctx.db, &gid, &filters, CSV_LIMIT, 0).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, %guild_id, "export des transactions");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "L'export est indisponible.");
        }
    };
    let body = build_csv(&rows, |user_id| creator_name(&ctx, guild_id, user_id));
    let today = chrono::Utc::now().with_timezone(&TIMEZONE).format("%Y-%m-%d");
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, format!(r#"attachment; filename="transactions-{gid}-{today}.csv""#)),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        body,
    )
        .into_response()
}

/// Neutralise les formules : Excel exécute une cellule qui commence par `=`,
/// `+`, `-` ou `@` (et LibreOffice aussi après tabulation ou retour chariot).
pub(super) fn neutralize(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{value}") } else { value.to_owned() }
}

/// Entoure de guillemets (doublés à l'intérieur) un champ qui contient le
/// séparateur, un guillemet ou un saut de ligne.
pub(super) fn quote(value: &str) -> String {
    if value.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Champ texte libre : neutralisé puis échappé.
pub(super) fn text_cell(value: &str) -> String {
    quote(&neutralize(value))
}

/// Nombre lisible par un Excel français : virgule décimale, sans séparateur de milliers.
pub(super) fn number_cell(value: f64) -> String {
    format_price(value).replace('\u{202f}', "")
}

fn build_csv(rows: &[HistoryRow], creator: impl Fn(&str) -> String) -> String {
    // BOM : sans lui, Excel lit le fichier en Windows-1252 et casse les accents.
    let mut csv = String::from("\u{feff}Date;Groupe;Type;Statut;Prix;Objets;Créé par\r\n");
    for row in rows {
        let status = Status::parse(&row.statut).map_or(row.statut.as_str(), |s| s.label());
        let items: Vec<String> = row.items.iter().map(|item| format!("{} x {}", item.nom, item.quantite)).collect();
        let cells = [
            format_date_label(row.created_at),
            text_cell(&row.groupe),
            type_label(&row.kind).to_owned(),
            text_cell(status),
            number_cell(row.prix),
            text_cell(&items.join(" | ")),
            text_cell(&creator(&row.created_by_id)),
        ];
        csv.push_str(&cells.join(";"));
        csv.push_str("\r\n");
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    async fn temp_db() -> (SqlitePool, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("bot-test-{}", uuid::Uuid::new_v4()));
        let pool = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        (pool, dir)
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DbTime {
        DbTime(Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap())
    }

    async fn insert(
        db: &SqlitePool,
        (id, guild): (&str, &str),
        groupe: &str,
        kind: &str,
        prix: f64,
        statut: &str,
        when: DbTime,
    ) {
        sqlx::query(
            r#"INSERT INTO "Transaction" ("id", "guildId", "channelId", "groupe", "type", "prix", "statut", "createdById", "createdAt", "updatedAt")
               VALUES (?, ?, 'c1', ?, ?, ?, ?, 'u1', ?, ?)"#,
        )
        .bind(id)
        .bind(guild)
        .bind(groupe)
        .bind(kind)
        .bind(prix)
        .bind(statut)
        .bind(when)
        .bind(when)
        .execute(db)
        .await
        .unwrap();
    }

    fn filters(query: &str) -> Filters {
        let mut q = HistoryQuery::default();
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap();
            let value = Some(value.to_owned());
            match key {
                "groupe" => q.groupe = value,
                "statut" => q.statut = value,
                "type" => q.kind = value,
                "du" => q.du = value,
                "au" => q.au = value,
                _ => {}
            }
        }
        Filters::from_query(&q)
    }

    #[tokio::test]
    async fn filters_and_aggregates_transactions() {
        let (db, dir) = temp_db().await;
        // 31/10 23:30 à Paris = 31/10 22:30 UTC (heure d'hiver) : reste en octobre.
        insert(&db, ("t1", "g1"), "Ballas", "VENTE", 12_500.0, "LIVRE_PAYE", at(2026, 10, 31, 22, 30)).await;
        insert(&db, ("t2", "g1"), "les ballas du sud", "ACHAT", 2_000.0, "PREPARATION", at(2026, 10, 2, 10, 0)).await;
        insert(&db, ("t3", "g1"), "Vagos", "VENTE", 900.0, "ANNULEE", at(2026, 9, 15, 10, 0)).await;
        // 1er novembre 00:30 à Paris = 31/10 23:30 UTC.
        insert(&db, ("t4", "g1"), "Vagos", "ACHAT", 4_000.0, "EN_ATTENTE_RDV", at(2026, 10, 31, 23, 30)).await;
        insert(&db, ("t5", "g1"), "100%_sûr", "VENTE", 50.0, "CLOTUREE", at(2026, 8, 1, 10, 0)).await;
        insert(&db, ("autre", "g2"), "Ballas", "VENTE", 99_999.0, "LIVRE_PAYE", at(2026, 10, 3, 10, 0)).await;
        for (id, tx, nom, quantite) in
            [("i1", "t1", "Pistolet", 2), ("i2", "t1", "Gilet", 1), ("i3", "t4", "Kevlar", 5)]
        {
            sqlx::query(
                r#"INSERT INTO "TransactionItem" ("id", "transactionId", "nom", "quantite", "prixUnitaire", "createdAt") VALUES (?, ?, ?, ?, 10.0, ?)"#,
            )
            .bind(id)
            .bind(tx)
            .bind(nom)
            .bind(quantite)
            .bind(DbTime::now())
            .execute(&db)
            .await
            .unwrap();
        }

        let all = amounts(&db, "g1", &Filters::default()).await.unwrap();
        let summary = summarize(&all);
        assert_eq!(summary.totals.count, 5);
        // L'annulée compte dans le nombre, pas dans les montants.
        assert_eq!(summary.totals.ventes, 12_550.0);
        assert_eq!(summary.totals.achats, 6_000.0);
        assert_eq!(summary.totals.solde(), 6_550.0);
        assert_eq!(summary.in_progress, 2);
        let months: Vec<_> = summary.months.iter().map(|(m, t)| (month_label(*m), t.count)).collect();
        assert_eq!(
            months,
            [
                ("novembre 2026".into(), 1),
                ("octobre 2026".into(), 2),
                ("septembre 2026".into(), 1),
                ("août 2026".into(), 1)
            ]
        );

        // Groupe insensible à la casse, jokers saisis pris littéralement.
        let ballas = filters("groupe=BALLAS");
        assert_eq!(amounts(&db, "g1", &ballas).await.unwrap().len(), 2);
        assert_eq!(amounts(&db, "g1", &filters("groupe=0%_")).await.unwrap().len(), 1);
        assert_eq!(amounts(&db, "g1", &filters("groupe=0%25")).await.unwrap().len(), 0);

        // Bornes de dates en jours entiers, heure de Paris, `au` inclus.
        let october = filters("du=2026-10-01&au=2026-10-31");
        let ids: Vec<String> = rows(&db, "g1", &october, 25, 0).await.unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["t1", "t2"]);
        assert_eq!(amounts(&db, "g1", &filters("du=2026-11-01")).await.unwrap().len(), 1);

        let combined = filters("type=VENTE&statut=LIVRE_PAYE&groupe=ball");
        assert_eq!(amounts(&db, "g1", &combined).await.unwrap().len(), 1);
        // Valeurs inconnues ignorées.
        assert_eq!(filters("statut=DROP&type=x&du=hier"), Filters::default());

        // Pagination et objets rattachés à la bonne transaction.
        let first = rows(&db, "g1", &Filters::default(), 2, 0).await.unwrap();
        assert_eq!(first.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["t4", "t1"]);
        assert_eq!(first[0].items.len(), 1);
        assert_eq!(first[1].items.iter().map(|i| i.nom.as_str()).collect::<Vec<_>>(), ["Pistolet", "Gilet"]);
        let second = rows(&db, "g1", &Filters::default(), 2, 2).await.unwrap();
        assert_eq!(second.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["t2", "t3"]);
        assert!(second.iter().all(|r| r.items.is_empty()));

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_filters_in_links() {
        let f = filters("groupe=Les Ballas&statut=PREPARATION&type=ACHAT&du=2026-10-01&au=2026-10-31");
        assert_eq!(f.query_string(), "groupe=Les%20Ballas&statut=PREPARATION&type=ACHAT&du=2026-10-01&au=2026-10-31");
        assert_eq!(Filters::default().query_string(), "");
    }

    #[test]
    fn escapes_csv_cells() {
        assert_eq!(text_cell("Ballas"), "Ballas");
        assert_eq!(text_cell("a;b"), "\"a;b\"");
        assert_eq!(text_cell("dit \"chef\""), "\"dit \"\"chef\"\"\"");
        assert_eq!(text_cell("ligne\nsuivante"), "\"ligne\nsuivante\"");
        assert_eq!(text_cell("=SOMME(A1)"), "'=SOMME(A1)");
        assert_eq!(text_cell("+33"), "'+33");
        assert_eq!(text_cell("-2"), "'-2");
        assert_eq!(text_cell("@cmd"), "'@cmd");
        assert_eq!(text_cell("=HYPERLINK(\"x\";\"y\")"), "\"'=HYPERLINK(\"\"x\"\";\"\"y\"\")\"");
        assert_eq!(number_cell(12_500.0), "12500");
        assert_eq!(number_cell(-1_500.5), "-1500,5");
    }

    #[test]
    fn builds_csv_with_bom_and_items() {
        let row = HistoryRow {
            id: "t1".into(),
            groupe: "=Ballas".into(),
            kind: "VENTE".into(),
            prix: 12_500.0,
            statut: "LIVRE_PAYE".into(),
            created_by_id: "42".into(),
            created_at: at(2026, 10, 16, 19, 40),
            items: vec![
                TransactionItem {
                    transaction_id: "t1".into(),
                    nom: "Pistolet".into(),
                    quantite: 2,
                    prix_unitaire: None,
                },
                TransactionItem { transaction_id: "t1".into(), nom: "Gilet".into(), quantite: 1, prix_unitaire: None },
            ],
        };
        let csv = build_csv(&[row], |_| "Élodie".into());
        let mut lines = csv.split("\r\n");
        assert_eq!(lines.next(), Some("\u{feff}Date;Groupe;Type;Statut;Prix;Objets;Créé par"));
        assert_eq!(
            lines.next(),
            Some("16/10/2026 21:40;'=Ballas;Vente;Livré et payé;12500;Pistolet x 2 | Gilet x 1;Élodie")
        );
        assert_eq!(lines.next(), Some(""));
    }

    #[test]
    fn formats_money_like_the_dashboard() {
        assert_eq!(money(12_500.0), "12\u{202f}500\u{202f}$");
        assert_eq!(money(-300.0), "-300\u{202f}$");
        assert_eq!(month_label((2026, 10)), "octobre 2026");
    }

    #[test]
    fn renders_transactions_page() {
        let amount = |kind: &str, prix: f64, statut: &str, when: DbTime| Amount {
            kind: kind.into(),
            prix,
            statut: statut.into(),
            created_at: when,
        };
        let summary = summarize(&[
            amount("VENTE", 12_500.0, "LIVRE_PAYE", at(2026, 10, 16, 19, 40)),
            amount("ACHAT", 4_000.0, "EN_ATTENTE_RDV", at(2026, 10, 14, 18, 5)),
            amount("VENTE", 2_750.0, "PREPARATION", at(2026, 10, 12, 20, 15)),
            amount("ACHAT", 18_000.0, "CLOTUREE", at(2026, 9, 28, 17, 0)),
            amount("VENTE", 900.0, "ANNULEE", at(2026, 9, 20, 21, 0)),
            amount("VENTE", 6_200.0, "LIVRE_PAYE", at(2026, 8, 30, 19, 0)),
        ]);
        let item = |nom: &str, quantite: i64, prix: Option<f64>| TransactionItem {
            transaction_id: String::new(),
            nom: nom.into(),
            quantite,
            prix_unitaire: prix,
        };
        let row =
            |groupe: &str, kind: &str, prix: f64, statut: &str, when: DbTime, items: Vec<TransactionItem>, by: &str| {
                row_view(
                    HistoryRow {
                        id: String::new(),
                        groupe: groupe.into(),
                        kind: kind.into(),
                        prix,
                        statut: statut.into(),
                        created_by_id: String::new(),
                        created_at: when,
                        items,
                    },
                    by.into(),
                )
            };
        let filters = Filters::default();
        let (statuts, types) = choices(&filters);
        let page = TransactionsPage {
            layout: Layout::bare("Transactions"),
            guild_id: "123".into(),
            guild_name: "Serveur RP".into(),
            groupe: String::new(),
            du: String::new(),
            au: String::new(),
            statuts,
            types,
            filtered: false,
            figures: figures(&summary),
            months: month_views(&summary),
            months_hidden: 0,
            rows: vec![
                row(
                    "Ballas",
                    "VENTE",
                    12_500.0,
                    "LIVRE_PAYE",
                    at(2026, 10, 16, 19, 40),
                    vec![item("Pistolet", 2, Some(4_500.0)), item("Gilet pare-balles", 1, Some(3_500.0))],
                    "Elodie",
                ),
                row(
                    "Vagos",
                    "ACHAT",
                    4_000.0,
                    "EN_ATTENTE_RDV",
                    at(2026, 10, 14, 18, 5),
                    vec![item("Kevlar", 10, None)],
                    "Marco",
                ),
                row("Families", "VENTE", 2_750.0, "PREPARATION", at(2026, 10, 12, 20, 15), vec![], "Sam"),
                row(
                    "Lost MC",
                    "ACHAT",
                    18_000.0,
                    "CLOTUREE",
                    at(2026, 9, 28, 17, 0),
                    vec![item("Moto", 2, Some(9_000.0))],
                    "Elodie",
                ),
                row("Marabunta", "VENTE", 900.0, "ANNULEE", at(2026, 9, 20, 21, 0), vec![], "Marco"),
            ],
            total: 6,
            page: 1,
            pages: 2,
            previous_href: None,
            next_href: Some("/stats/123/transactions?page=2".into()),
            csv_href: "/stats/123/transactions.csv".into(),
        };
        let html = page.render().unwrap();
        assert!(html.contains("octobre 2026") && html.contains("Exporter en CSV"));
        assert!(html.contains("Pistolet") && html.contains("Livré et payé") && html.contains("12\u{202f}500\u{202f}$"));
        if let Ok(path) = std::env::var("TRANSACTIONS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, crate::web::ASSET_VERSION),
                &format!("<style>{}</style>", crate::web::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
