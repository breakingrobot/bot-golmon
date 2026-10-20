//! Historique des transactions d'un serveur : filtres, totaux par mois, détail
//! des objets et export CSV. Accès : `transaction.historique`.

use crate::{
    context::Ctx,
    discord::members,
    modules::{
        like_contains,
        transactions::model::{Status, TYPE_CHOICES, TransactionItem, type_label},
    },
    util::{
        format::format_price,
        time::{DbTime, TIMEZONE, format_date_label, paris_day_start, parse_iso_day},
    },
};
use anyhow::Result;
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use sqlx::{FromRow, SqlitePool};
use std::collections::{BTreeMap, HashMap};
use twilight_model::id::{Id, marker::GuildMarker};

pub(crate) const PAGE_SIZE: i64 = 25;
const CSV_LIMIT: i64 = 10_000;
pub(crate) const MONTHS_SHOWN: usize = 12;

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    /// Texte cherché dans le groupe.
    groupe: Option<String>,
    /// Statut (`PREPARATION`…).
    statut: Option<String>,
    /// `ACHAT` ou `VENTE`.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Premier jour (`YYYY-MM-DD`, heure de Paris).
    du: Option<String>,
    /// Dernier jour inclus (`YYYY-MM-DD`).
    au: Option<String>,
    /// Page (1 = première). En texte : une valeur invalide ramène à la première page plutôt qu'à une erreur 400.
    pub(crate) page: Option<String>,
}

/// Filtres validés : une valeur inconnue est ignorée, jamais injectée telle quelle.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Filters {
    groupe: String,
    statut: Option<Status>,
    kind: Option<&'static str>,
    du: Option<NaiveDate>,
    au: Option<NaiveDate>,
}

impl Filters {
    pub(crate) fn from_query(query: &HistoryQuery) -> Self {
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
pub(crate) struct Amount {
    #[sqlx(rename = "type")]
    kind: String,
    prix: f64,
    statut: String,
    created_at: DbTime,
}

#[derive(Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub(crate) struct HistoryRow {
    pub(crate) id: String,
    pub(crate) groupe: String,
    #[sqlx(rename = "type")]
    pub(crate) kind: String,
    pub(crate) prix: f64,
    pub(crate) statut: String,
    pub(crate) created_by_id: String,
    pub(crate) created_at: DbTime,
    pub(crate) note: Option<String>,
    #[sqlx(skip)]
    pub(crate) items: Vec<TransactionItem>,
}

pub(crate) async fn amounts(db: &SqlitePool, guild_id: &str, filters: &Filters) -> Result<Vec<Amount>> {
    let (clause, binds) = filters.where_clause(guild_id);
    let sql = format!(r#"SELECT "type", "prix", "statut", "createdAt" FROM "Transaction" WHERE {clause}"#);
    let mut query = sqlx::query_as::<_, Amount>(&sql);
    for value in binds {
        query = query.bind(value);
    }
    Ok(query.fetch_all(db).await?)
}

/// Transactions filtrées, les plus récentes d'abord, avec leurs objets.
pub(crate) async fn rows(
    db: &SqlitePool,
    guild_id: &str,
    filters: &Filters,
    limit: i64,
    offset: i64,
) -> Result<Vec<HistoryRow>> {
    let (clause, binds) = filters.where_clause(guild_id);
    let selection = format!(
        r#"SELECT "id", "groupe", "type", "prix", "statut", "createdById", "createdAt", "note" FROM "Transaction"
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
        r#"SELECT "id", "transactionId", "nom", "quantite", "prixUnitaire" FROM "TransactionItem"
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
pub(crate) struct Totals {
    pub(crate) count: usize,
    pub(crate) achats: f64,
    pub(crate) ventes: f64,
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

    pub(crate) fn solde(&self) -> f64 {
        self.ventes - self.achats
    }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) totals: Totals,
    pub(crate) in_progress: usize,
    /// Mois (année, mois) en heure de Paris, du plus récent au plus ancien.
    pub(crate) months: Vec<((i32, u32), Totals)>,
}

pub(crate) fn summarize(amounts: &[Amount]) -> Summary {
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

/// Export CSV des transactions filtrées (page et API).
pub(crate) async fn csv_response(ctx: &Ctx, guild_id: Id<GuildMarker>, query: &HistoryQuery) -> Response {
    let filters = Filters::from_query(query);
    let gid = guild_id.to_string();
    let rows = match rows(&ctx.db, &gid, &filters, CSV_LIMIT, 0).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, %guild_id, "export des transactions");
            return (StatusCode::INTERNAL_SERVER_ERROR, "L'export est indisponible.").into_response();
        }
    };
    let body = build_csv(&rows, |user_id| {
        user_id
            .parse()
            .ok()
            .and_then(|id| members::display_name(ctx, guild_id, id))
            .unwrap_or_else(|| user_id.to_owned())
    });
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
        let months: Vec<_> = summary.months.iter().map(|((y, m), t)| (format!("{y}-{m:02}"), t.count)).collect();
        assert_eq!(
            months,
            [("2026-11".into(), 1), ("2026-10".into(), 2), ("2026-09".into(), 1), ("2026-08".into(), 1)]
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
            note: None,
            items: vec![
                TransactionItem {
                    id: "i1".into(),
                    transaction_id: "t1".into(),
                    nom: "Pistolet".into(),
                    quantite: 2,
                    prix_unitaire: None,
                },
                TransactionItem {
                    id: "i2".into(),
                    transaction_id: "t1".into(),
                    nom: "Gilet".into(),
                    quantite: 1,
                    prix_unitaire: None,
                },
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
}
