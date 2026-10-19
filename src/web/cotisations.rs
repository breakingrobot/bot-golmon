//! Cotisations d'un serveur : totaux propre / sale, classement des membres,
//! détail et export CSV. Accès : `cotisation.stats`.

use super::transactions::{number_cell, text_cell};
use crate::{
    context::Ctx,
    discord::members,
    modules::cotisations::service::{self, Cotisation, Filter},
    util::time::{DbTime, format_day_label},
};
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Duration;
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

pub(crate) const PAGE_SIZE: i64 = 50;
const CSV_LIMIT: i64 = 10_000;

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CotisationQuery {
    /// Les N derniers jours (absent : depuis le début).
    jours: Option<String>,
    /// Identifiant Discord d'un membre.
    membre: Option<String>,
    /// Page (1 = première).
    page: Option<String>,
}

pub(crate) struct Parsed {
    pub(crate) days: Option<i64>,
    pub(crate) member: Option<String>,
    pub(crate) page: i64,
}

impl CotisationQuery {
    /// Valeurs invalides ignorées plutôt que refusées.
    pub(crate) fn parse(&self) -> Parsed {
        let days = self.jours.as_deref().and_then(|d| d.trim().parse::<i64>().ok()).filter(|d| (1..=3650).contains(d));
        let member = self
            .membre
            .as_deref()
            .map(str::trim)
            .filter(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit()))
            .map(str::to_owned);
        let page = self.page.as_deref().and_then(|p| p.parse().ok()).unwrap_or(1).max(1);
        Parsed { days, member, page }
    }
}

impl Parsed {
    pub(crate) fn filter(&self) -> Filter {
        Filter {
            since: self.days.map(|days| DbTime(DbTime::now().0 - Duration::days(days))),
            until: None,
            user_id: self.member.clone(),
        }
    }
}

fn name_of(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    user_id.parse().ok().and_then(|id| members::display_name(ctx, guild_id, id)).unwrap_or_else(|| user_id.to_owned())
}

fn build_csv(rows: &[Cotisation], name: impl Fn(&str) -> String) -> String {
    // BOM : sans lui, Excel lit le fichier en Windows-1252 et casse les accents.
    let mut csv = String::from("\u{feff}Date;Membre;Propre;Sale;Total;Description;Déclarée par\r\n");
    for row in rows {
        let line = [
            format_day_label(row.date),
            text_cell(&name(&row.user_id)),
            number_cell(row.propre as f64),
            number_cell(row.sale as f64),
            number_cell(row.total() as f64),
            text_cell(&row.description),
            text_cell(&name(&row.created_by_id)),
        ];
        csv.push_str(&line.join(";"));
        csv.push_str("\r\n");
    }
    csv
}

/// Export CSV des cotisations filtrées (page et API).
pub(crate) async fn csv_response(ctx: &Ctx, guild_id: Id<GuildMarker>, query: &CotisationQuery) -> Response {
    let rows = match service::list(&ctx.db, &guild_id.to_string(), &query.parse().filter(), CSV_LIMIT, 0).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, "export des cotisations");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Export indisponible").into_response();
        }
    };
    let csv = build_csv(&rows, |id| name_of(ctx, guild_id, id));
    let filename = format!("cotisations-{guild_id}-{}.csv", chrono::Utc::now().format("%Y-%m-%d"));
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        csv,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filters() {
        let query = CotisationQuery { jours: Some("30".into()), membre: Some("123".into()), page: Some("2".into()) };
        let parsed = query.parse();
        assert_eq!((parsed.days, parsed.member.as_deref(), parsed.page), (Some(30), Some("123"), 2));
        let bad = CotisationQuery { jours: Some("-4".into()), membre: Some("1 OR 1".into()), page: Some("x".into()) };
        let parsed = bad.parse();
        assert_eq!((parsed.days, parsed.member.as_deref(), parsed.page), (None, None, 1));
    }

    #[test]
    fn exports_csv_safely() {
        let row = Cotisation {
            id: "1".into(),
            user_id: "42".into(),
            propre: 1500,
            sale: 0,
            description: "=HYPERLINK(\"x\");vente".into(),
            date: DbTime::now(),
            created_by_id: "42".into(),
        };
        let csv = build_csv(&[row], |_| "Elodie".into());
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains(";Elodie;1500;0;1500;\"'=HYPERLINK(\"\"x\"\");vente\";Elodie\r\n"));
    }
}
