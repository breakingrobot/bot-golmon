//! Coffres d'un serveur : stock par propriété, activité par joueur, mouvements
//! filtrables et export CSV. Accès : `coffre.voir`.

use super::{
    Access, MaybeViewer, Viewer,
    pages::{self, Layout},
    transactions::{number_cell, text_cell},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::coffres::{
        commands::{group_stock, property_label},
        parser::{Kind, item_label},
        service::{self, Filter, MovementRow},
    },
    modules::permissions::Perm,
    util::time::{DbTime, format_date_label},
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use chrono::Duration;
use serde::Deserialize;
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};

const PAGE_SIZE: i64 = 50;
const CSV_LIMIT: i64 = 20_000;
/// Période affichée sans choix explicite.
const DEFAULT_DAYS: i64 = 30;

const PERIODS: [(Option<i64>, &str); 5] =
    [(Some(1), "24 h"), (Some(7), "7 jours"), (Some(30), "30 jours"), (Some(90), "90 jours"), (None, "Tout")];

#[derive(Debug, Default, Deserialize)]
pub struct CoffreQuery {
    propriete: Option<String>,
    objet: Option<String>,
    joueur: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    jours: Option<String>,
    page: Option<String>,
}

#[derive(Clone, Default)]
struct Parsed {
    property: Option<String>,
    item: Option<String>,
    player: Option<String>,
    kind: Option<Kind>,
    days: Option<i64>,
    page: i64,
}

fn clean(value: Option<&String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty() && v.len() <= 100)
}

impl CoffreQuery {
    fn parse(&self) -> Parsed {
        Parsed {
            property: clean(self.propriete.as_ref()),
            item: clean(self.objet.as_ref()),
            player: clean(self.joueur.as_ref()).filter(|p| p.bytes().all(|b| b.is_ascii_digit())),
            kind: self.kind.as_deref().and_then(Kind::parse),
            // 30 jours par défaut : l'historique complet (« tout ») est à demander.
            days: match self.jours.as_deref() {
                None => Some(DEFAULT_DAYS),
                Some("tout") => None,
                Some(d) => Some(d.parse().ok().filter(|d| (1..=3650).contains(d)).unwrap_or(DEFAULT_DAYS)),
            },
            page: self.page.as_deref().and_then(|p| p.parse().ok()).unwrap_or(1).max(1),
        }
    }
}

impl Parsed {
    fn filter(&self) -> Filter {
        Filter {
            property_id: self.property.clone(),
            item: self.item.clone(),
            player_game_ids: self.player.clone().map(|p| vec![p]),
            kind: self.kind,
            since: self.days.map(|d| DbTime(DbTime::now().0 - Duration::days(d))),
        }
    }

    fn query(&self, page: Option<i64>) -> String {
        let mut parts = Vec::new();
        let encode = super::auth::url_encode;
        if let Some(v) = &self.property {
            parts.push(format!("propriete={}", encode(v)));
        }
        if let Some(v) = &self.item {
            parts.push(format!("objet={}", encode(v)));
        }
        if let Some(v) = &self.player {
            parts.push(format!("joueur={v}"));
        }
        if let Some(kind) = self.kind {
            parts.push(format!("type={}", kind.as_str().to_lowercase()));
        }
        match self.days {
            Some(DEFAULT_DAYS) => {}
            Some(days) => parts.push(format!("jours={days}")),
            None => parts.push("jours=tout".to_owned()),
        }
        if let Some(page) = page.filter(|p| *p > 1) {
            parts.push(format!("page={page}"));
        }
        if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
    }
}

struct Choice {
    value: String,
    label: String,
    selected: bool,
}

struct Period {
    label: &'static str,
    href: String,
    active: bool,
}

struct StockItem {
    label: String,
    quantity: i64,
    negative: bool,
}

struct StockCard {
    name: String,
    href: String,
    items: Vec<StockItem>,
}

struct PlayerRow {
    href: String,
    name: String,
    member: Option<String>,
    deposited: i64,
    withdrawn: i64,
    count: i64,
}

struct MovementView {
    time_iso: String,
    time: String,
    deposit: bool,
    item: String,
    amount: i64,
    player: String,
    property: String,
}

#[derive(Template)]
#[template(path = "coffres.html")]
struct CoffresPage {
    layout: Layout,
    guild_name: String,
    properties: Vec<Choice>,
    items: Vec<Choice>,
    kinds: Vec<Choice>,
    player_name: Option<String>,
    days: String,
    periods: Vec<Period>,
    stock: Vec<StockCard>,
    players: Vec<PlayerRow>,
    movements: Vec<MovementView>,
    total: i64,
    csv_href: String,
    page: i64,
    pages: i64,
    previous_href: Option<String>,
    next_href: Option<String>,
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
    if !access.can(guild_id, Perm::CoffreVoir) {
        return Err(Box::new(pages::forbidden(ctx, &viewer, &access)));
    }
    let name = access.guild_name(guild_id);
    Ok((viewer, access, guild_id, name))
}

fn member_name(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    user_id.parse().ok().and_then(|id| members::display_name(ctx, guild_id, id)).unwrap_or_else(|| user_id.to_owned())
}

fn movement_view(row: &MovementRow, names: &HashMap<String, String>) -> MovementView {
    MovementView {
        time_iso: row.at.0.to_rfc3339(),
        time: format_date_label(row.at),
        deposit: row.kind() == Kind::Deposit,
        item: item_label(&row.item),
        amount: row.amount,
        player: row.player_name.clone(),
        property: property_label(names, &row.property_id),
    }
}

pub async fn page(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<CoffreQuery>,
) -> Response {
    let (viewer, access, guild_id, guild_name) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let parsed = query.parse();
    let filter = parsed.filter();
    let guild = guild_id.to_string();

    let data = async {
        let properties = service::properties(&ctx.db, &guild).await?;
        let items = service::items(&ctx.db, &guild).await?;
        let stock = service::stock(&ctx.db, &guild, parsed.property.as_deref()).await?;
        let players = service::by_player(&ctx.db, &guild, &filter).await?;
        let links = service::player_links(&ctx.db, &guild).await?;
        let total = service::count(&ctx.db, &guild, &filter).await?;
        let pages_count = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
        let page = parsed.page.min(pages_count);
        let rows = service::movements(&ctx.db, &guild, &filter, PAGE_SIZE, (page - 1) * PAGE_SIZE).await?;
        anyhow::Ok((properties, items, stock, players, links, total, page, pages_count, rows))
    }
    .await;
    let (properties, items, stock, players, links, total, page, pages_count, rows) = match data {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(?error, "lecture des coffres");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Coffres indisponibles.");
        }
    };

    let names: HashMap<String, String> =
        properties.iter().filter_map(|(id, name)| name.clone().map(|n| (id.clone(), n))).collect();
    let base = format!("/stats/{guild}/coffres");
    let link = |change: &dyn Fn(&mut Parsed)| {
        let mut next = parsed.clone();
        next.page = 1;
        change(&mut next);
        format!("{base}{}", next.query(None))
    };

    let stock = group_stock(stock)
        .into_iter()
        .map(|(property, lines)| StockCard {
            name: property_label(&names, &property),
            href: link(&|p| p.property = Some(property.clone())),
            items: lines
                .iter()
                .map(|l| StockItem { label: item_label(&l.item), quantity: l.quantity(), negative: l.quantity() < 0 })
                .collect(),
        })
        .collect();
    let player_name = parsed
        .player
        .as_ref()
        .map(|id| players.iter().find(|(pid, ..)| pid == id).map_or_else(|| id.clone(), |(_, name, ..)| name.clone()));
    let players = players
        .into_iter()
        .map(|(id, name, deposited, withdrawn, count)| PlayerRow {
            href: link(&|p| p.player = Some(id.clone()).filter(|i| i.bytes().all(|b| b.is_ascii_digit()))),
            member: links.get(&id).map(|user| member_name(&ctx, guild_id, user)),
            name,
            deposited,
            withdrawn,
            count,
        })
        .collect();

    let properties = std::iter::once(Choice {
        value: String::new(),
        label: "Toutes les propriétés".into(),
        selected: parsed.property.is_none(),
    })
    .chain(properties.iter().map(|(id, _)| Choice {
        value: id.clone(),
        label: property_label(&names, id),
        selected: parsed.property.as_deref() == Some(id.as_str()),
    }))
    .collect();
    let items = std::iter::once(Choice {
        value: String::new(),
        label: "Tous les objets".into(),
        selected: parsed.item.is_none(),
    })
    .chain(items.iter().map(|item| Choice {
        value: item.clone(),
        label: item_label(item),
        selected: parsed.item.as_deref() == Some(item.as_str()),
    }))
    .collect();
    let kinds = [
        ("", "Dépôts et retraits", None),
        ("deposit", "Dépôts", Some(Kind::Deposit)),
        ("withdraw", "Retraits", Some(Kind::Withdraw)),
    ]
    .into_iter()
    .map(|(value, label, kind)| Choice { value: value.into(), label: label.into(), selected: parsed.kind == kind })
    .collect();
    let periods = PERIODS
        .iter()
        .map(|(days, label)| Period { label, href: link(&|p| p.days = *days), active: parsed.days == *days })
        .collect();

    pages::render(&CoffresPage {
        layout: Layout::new(&ctx, format!("Coffres · {guild_name}"), Some(&viewer), &access, "/cotisations"),
        guild_name,
        properties,
        items,
        kinds,
        player_name,
        days: parsed.days.map_or_else(|| "tout".to_owned(), |d| d.to_string()),
        periods,
        stock,
        players,
        movements: rows.iter().map(|r| movement_view(r, &names)).collect(),
        total,
        csv_href: format!("{base}.csv{}", parsed.query(None)),
        page,
        pages: pages_count,
        previous_href: (page > 1).then(|| format!("{base}{}", parsed.query(Some(page - 1)))),
        next_href: (page < pages_count).then(|| format!("{base}{}", parsed.query(Some(page + 1)))),
    })
}

fn build_csv(rows: &[MovementRow], names: &HashMap<String, String>) -> String {
    let mut csv = String::from("\u{feff}Date;Propriété;Coffre;Objet;Type;Quantité;Joueur;Identifiant jeu\r\n");
    for row in rows {
        let line = [
            format_date_label(row.at),
            text_cell(&property_label(names, &row.property_id)),
            row.chest.map(|c| c.to_string()).unwrap_or_default(),
            text_cell(&row.item),
            row.kind().label().to_owned(),
            number_cell(row.amount as f64),
            text_cell(&row.player_name),
            row.player_game_id.clone().unwrap_or_default(),
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
    Query(query): Query<CoffreQuery>,
) -> Response {
    let (_, _, guild_id, _) = match authorize(&ctx, viewer, &uri, guild_id).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let guild = guild_id.to_string();
    let data = async {
        let rows = service::movements(&ctx.db, &guild, &query.parse().filter(), CSV_LIMIT, 0).await?;
        let names: HashMap<String, String> = service::properties(&ctx.db, &guild)
            .await?
            .into_iter()
            .filter_map(|(id, name)| name.map(|n| (id, n)))
            .collect();
        anyhow::Ok(build_csv(&rows, &names))
    }
    .await;
    match data {
        Ok(csv) => {
            let filename = format!("coffres-{guild}-{}.csv", chrono::Utc::now().format("%Y-%m-%d"));
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
        Err(error) => {
            tracing::error!(?error, "export des coffres");
            (StatusCode::INTERNAL_SERVER_ERROR, "Export indisponible").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filters() {
        let query = CoffreQuery {
            propriete: Some("8740".into()),
            objet: Some("repair_kit".into()),
            joueur: Some("340772".into()),
            kind: Some("withdraw".into()),
            jours: Some("7".into()),
            page: Some("3".into()),
        };
        let parsed = query.parse();
        assert_eq!(
            parsed.query(Some(2)),
            "?propriete=8740&objet=repair_kit&joueur=340772&type=withdraw&jours=7&page=2"
        );
        let bad = CoffreQuery { joueur: Some("1 OR 1".into()), kind: Some("x".into()), ..Default::default() }.parse();
        assert_eq!((bad.player, bad.kind, bad.page), (None, None, 1));
    }

    #[test]
    fn renders_coffres_page() {
        let names: HashMap<String, String> = [("8740".to_owned(), "Planque du port".to_owned())].into();
        let row = |item: &str, amount: i64, kind: &str| MovementRow {
            property_id: "8740".into(),
            chest: Some(1),
            item: item.into(),
            amount,
            kind: kind.into(),
            player_name: "Kyle Ian Bappy Duggan".into(),
            player_game_id: Some("340772".into()),
            at: DbTime::now(),
        };
        let rows = [row("repair_kit", 8, "DEPOSIT"), row("repair_kit", 7, "WITHDRAW"), row("lockpick", 3, "DEPOSIT")];
        let page = CoffresPage {
            layout: Layout::bare("Coffres"),
            guild_name: "Serveur RP".into(),
            properties: vec![Choice { value: "8740".into(), label: "Planque du port".into(), selected: false }],
            items: vec![Choice { value: "repair_kit".into(), label: "Repair kit".into(), selected: false }],
            kinds: vec![Choice { value: String::new(), label: "Dépôts et retraits".into(), selected: true }],
            player_name: None,
            days: String::new(),
            periods: PERIODS.iter().map(|(d, label)| Period { label, href: "#".into(), active: d.is_none() }).collect(),
            stock: vec![StockCard {
                name: "Planque du port".into(),
                href: "#".into(),
                items: vec![
                    StockItem { label: "Repair kit".into(), quantity: 12, negative: false },
                    StockItem { label: "Lockpick".into(), quantity: 3, negative: false },
                    StockItem { label: "Medikit".into(), quantity: -2, negative: true },
                ],
            }],
            players: vec![PlayerRow {
                href: "#".into(),
                name: "Kyle Ian Bappy Duggan".into(),
                member: Some("Elodie".into()),
                deposited: 21,
                withdrawn: 9,
                count: 5,
            }],
            movements: rows.iter().map(|r| movement_view(r, &names)).collect(),
            total: 3,
            csv_href: "#".into(),
            page: 1,
            pages: 1,
            previous_href: None,
            next_href: None,
        };
        let html = page.render().unwrap();
        assert!(html.contains("Planque du port") && html.contains("Repair kit") && html.contains("Elodie"));
        let csv = build_csv(&rows, &names);
        assert!(csv.contains(";Planque du port;1;repair_kit;Retrait;7;Kyle Ian Bappy Duggan;340772\r\n"));
        if let Ok(path) = std::env::var("COFFRES_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, super::super::ASSET_VERSION),
                &format!("<style>{}</style>", super::super::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
