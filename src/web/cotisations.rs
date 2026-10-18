//! Cotisations d'un serveur : totaux propre / sale, classement des membres,
//! détail et export CSV. Accès : gradés, leads et « Gérer le serveur ».

use super::{
    Access, MaybeViewer, Viewer,
    pages::{self, Layout},
    transactions::{number_cell, text_cell},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::cotisations::{
        money,
        service::{self, Cotisation, Filter, Totals},
    },
    util::time::{DbTime, format_day_label},
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Redirect, Response},
};
use chrono::Duration;
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

const PAGE_SIZE: i64 = 50;
const CSV_LIMIT: i64 = 10_000;
/// Périodes proposées (jours), `None` = depuis le début.
const PERIODS: [(Option<i64>, &str); 5] =
    [(Some(7), "7 jours"), (Some(30), "30 jours"), (Some(90), "90 jours"), (Some(365), "1 an"), (None, "Tout")];

#[derive(Debug, Default, Deserialize)]
pub struct CotisationQuery {
    jours: Option<String>,
    membre: Option<String>,
    page: Option<String>,
}

struct Parsed {
    days: Option<i64>,
    member: Option<String>,
    page: i64,
}

impl CotisationQuery {
    /// Valeurs invalides ignorées plutôt que refusées.
    fn parse(&self) -> Parsed {
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
    fn filter(&self) -> Filter {
        Filter {
            since: self.days.map(|days| DbTime(DbTime::now().0 - Duration::days(days))),
            until: None,
            user_id: self.member.clone(),
        }
    }

    fn query(&self, page: Option<i64>) -> String {
        let mut parts = Vec::new();
        if let Some(days) = self.days {
            parts.push(format!("jours={days}"));
        }
        if let Some(member) = &self.member {
            parts.push(format!("membre={member}"));
        }
        if let Some(page) = page.filter(|p| *p > 1) {
            parts.push(format!("page={page}"));
        }
        if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
    }
}

struct Figure {
    label: &'static str,
    value: String,
    tone: &'static str,
}

struct MemberRow {
    rank: usize,
    href: String,
    name: String,
    propre: String,
    sale: String,
    total: String,
    count: i64,
    share: i64,
}

struct EntryRow {
    date: String,
    member: String,
    propre: String,
    sale: String,
    description: String,
    declared_by: Option<String>,
}

struct Period {
    label: &'static str,
    href: String,
    active: bool,
}

#[derive(Template)]
#[template(path = "cotisations.html")]
struct CotisationsPage {
    layout: Layout,
    guild_name: String,
    member_name: Option<String>,
    clear_member_href: String,
    periods: Vec<Period>,
    figures: Vec<Figure>,
    members: Vec<MemberRow>,
    entries: Vec<EntryRow>,
    csv_href: String,
    coffres_href: String,
    page: i64,
    pages: i64,
    previous_href: Option<String>,
    next_href: Option<String>,
}

fn name_of(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    user_id.parse().ok().and_then(|id| members::display_name(ctx, guild_id, id)).unwrap_or_else(|| user_id.to_owned())
}

fn figures(totals: &Totals, contributors: usize) -> Vec<Figure> {
    vec![
        Figure { label: "Total", value: money(totals.total()), tone: "text-staff" },
        Figure { label: "Argent propre", value: money(totals.propre), tone: "text-ink" },
        Figure { label: "Argent sale", value: money(totals.sale), tone: "text-internal" },
        Figure { label: "Cotisations", value: totals.count.to_string(), tone: "text-ink" },
        Figure { label: "Contributeurs", value: contributors.to_string(), tone: "text-ink" },
    ]
}

async fn authorize(
    ctx: &Ctx,
    viewer: MaybeViewer,
    uri: &Uri,
    guild_id: u64,
) -> Result<(Viewer, Access, Id<GuildMarker>, String), Box<Response>> {
    let path = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let (viewer, access) = pages::require_viewer(ctx, viewer, path).await?;
    let Some(guild_id) = Id::<GuildMarker>::new_checked(guild_id) else {
        return Err(Box::new(pages::not_found().await));
    };
    if !access.is_officer_of(guild_id) {
        return Err(Box::new(pages::forbidden(ctx, &viewer, &access)));
    }
    let name = access.officer_guilds.iter().find(|(id, _)| *id == guild_id).map(|(_, n)| n.clone()).unwrap_or_default();
    Ok((viewer, access, guild_id, name))
}

/// `/cotisations` : le serveur de l'officier, ou l'accueil s'il en a plusieurs.
pub async fn index(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri) -> Response {
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, uri.path()).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    match access.officer_guilds.as_slice() {
        [] => pages::forbidden(&ctx, &viewer, &access),
        [(id, _)] => Redirect::to(&format!("/stats/{id}/cotisations")).into_response(),
        _ => Redirect::to("/").into_response(),
    }
}

pub async fn page(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<CotisationQuery>,
) -> Response {
    let (viewer, access, guild_id, guild_name) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let parsed = query.parse();
    let filter = parsed.filter();
    let guild = guild_id.to_string();

    let data = async {
        let totals = service::totals(&ctx.db, &guild, &filter).await?;
        let by_member = service::by_member(&ctx.db, &guild, &filter).await?;
        let pages_count = ((totals.count + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
        let page = parsed.page.min(pages_count);
        let rows = service::list(&ctx.db, &guild, &filter, PAGE_SIZE, (page - 1) * PAGE_SIZE).await?;
        anyhow::Ok((totals, by_member, rows, page, pages_count))
    }
    .await;
    let (totals, by_member, rows, page, pages_count) = match data {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(?error, "lecture des cotisations");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Cotisations indisponibles.");
        }
    };

    let base = format!("/stats/{guild}/cotisations");
    let grand_total = totals.total().max(1);
    let members = by_member
        .iter()
        .enumerate()
        .map(|(index, (user, t))| {
            let link = Parsed { days: parsed.days, member: Some(user.clone()), page: 1 };
            MemberRow {
                rank: index + 1,
                href: format!("{base}{}", link.query(None)),
                name: name_of(&ctx, guild_id, user),
                propre: money(t.propre),
                sale: money(t.sale),
                total: money(t.total()),
                count: t.count,
                share: (t.total() * 100 + grand_total / 2) / grand_total,
            }
        })
        .collect();
    let entries = rows
        .iter()
        .map(|c: &Cotisation| EntryRow {
            date: format_day_label(c.date),
            member: name_of(&ctx, guild_id, &c.user_id),
            propre: if c.propre > 0 { money(c.propre) } else { "—".to_owned() },
            sale: if c.sale > 0 { money(c.sale) } else { "—".to_owned() },
            description: c.description.clone(),
            declared_by: (c.created_by_id != c.user_id).then(|| name_of(&ctx, guild_id, &c.created_by_id)),
        })
        .collect();
    let periods = PERIODS
        .iter()
        .map(|(days, label)| Period {
            label,
            href: format!("{base}{}", Parsed { days: *days, member: parsed.member.clone(), page: 1 }.query(None)),
            active: parsed.days == *days,
        })
        .collect();

    pages::render(&CotisationsPage {
        layout: Layout::new(&ctx, format!("Cotisations · {guild_name}"), Some(&viewer), &access, "/cotisations"),
        guild_name,
        member_name: parsed.member.as_deref().map(|m| name_of(&ctx, guild_id, m)),
        clear_member_href: format!("{base}{}", Parsed { days: parsed.days, member: None, page: 1 }.query(None)),
        periods,
        figures: figures(&totals, by_member.len()),
        members,
        entries,
        csv_href: format!("{base}.csv{}", parsed.query(None)),
        coffres_href: format!("/stats/{guild}/coffres"),
        page,
        pages: pages_count,
        previous_href: (page > 1).then(|| format!("{base}{}", parsed.query(Some(page - 1)))),
        next_href: (page < pages_count).then(|| format!("{base}{}", parsed.query(Some(page + 1)))),
    })
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

pub async fn export(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<CotisationQuery>,
) -> Response {
    let (_, _, guild_id, _) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let rows = match service::list(&ctx.db, &guild_id.to_string(), &query.parse().filter(), CSV_LIMIT, 0).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(?error, "export des cotisations");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Export indisponible").into_response();
        }
    };
    let csv = build_csv(&rows, |id| name_of(&ctx, guild_id, id));
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
    fn parses_filters_and_builds_links() {
        let query = CotisationQuery { jours: Some("30".into()), membre: Some("123".into()), page: Some("2".into()) };
        let parsed = query.parse();
        assert_eq!((parsed.days, parsed.member.as_deref(), parsed.page), (Some(30), Some("123"), 2));
        assert_eq!(parsed.query(Some(3)), "?jours=30&membre=123&page=3");
        let bad = CotisationQuery { jours: Some("-4".into()), membre: Some("1 OR 1".into()), page: Some("x".into()) };
        let parsed = bad.parse();
        assert_eq!((parsed.days, parsed.member.as_deref(), parsed.page), (None, None, 1));
        assert_eq!(parsed.query(None), "");
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

    #[test]
    fn renders_cotisations_page() {
        let page = CotisationsPage {
            layout: Layout::bare("Cotisations"),
            guild_name: "Serveur RP".into(),
            member_name: None,
            clear_member_href: "/stats/1/cotisations".into(),
            periods: PERIODS
                .iter()
                .map(|(days, label)| Period { label, href: "#".into(), active: *days == Some(30) })
                .collect(),
            figures: figures(&Totals { count: 14, propre: 182_500, sale: 96_000 }, 3),
            members: vec![
                MemberRow {
                    rank: 1,
                    href: "#".into(),
                    name: "Elodie".into(),
                    propre: money(120_000),
                    sale: money(40_000),
                    total: money(160_000),
                    count: 6,
                    share: 57,
                },
                MemberRow {
                    rank: 2,
                    href: "#".into(),
                    name: "Marco".into(),
                    propre: money(50_000),
                    sale: money(56_000),
                    total: money(106_000),
                    count: 5,
                    share: 38,
                },
                MemberRow {
                    rank: 3,
                    href: "#".into(),
                    name: "Sam".into(),
                    propre: money(12_500),
                    sale: "0\u{202f}$".into(),
                    total: money(12_500),
                    count: 3,
                    share: 5,
                },
            ],
            entries: vec![
                EntryRow {
                    date: "17/10/2026".into(),
                    member: "Elodie".into(),
                    propre: money(25_000),
                    sale: "—".into(),
                    description: "Livraison de pièces au garage".into(),
                    declared_by: None,
                },
                EntryRow {
                    date: "16/10/2026".into(),
                    member: "Marco".into(),
                    propre: "—".into(),
                    sale: money(18_000),
                    description: "Revente de marchandises".into(),
                    declared_by: Some("Elodie".into()),
                },
            ],
            csv_href: "#".into(),
            coffres_href: "#".into(),
            page: 1,
            pages: 1,
            previous_href: None,
            next_href: None,
        };
        let html = page.render().unwrap();
        assert!(html.contains("Argent sale") && html.contains("Elodie") && html.contains("déclarée par Elodie"));
        if let Ok(path) = std::env::var("COTISATIONS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, super::super::ASSET_VERSION),
                &format!("<style>{}</style>", super::super::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
