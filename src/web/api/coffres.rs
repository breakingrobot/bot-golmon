//! Coffres : stock, mouvements, joueurs, évolution d'un objet, liens et noms
//! (lecture `coffre.voir`, écriture `coffre.gerer`, comme `/coffre`).

use super::{
    ApiError, ApiQuery, ApiResult, JsonBody, MemberRef, Members, Page, Session, clamp_page, iso_db, json, no_content,
    parse_user,
    series::{self, paris_date},
};
use crate::{
    context::Context,
    modules::{
        audit::{self, Action, Entry},
        coffres::{
            commands::group_stock,
            parser::{Kind, item_label},
            service::{self, Filter},
        },
        permissions::Perm,
    },
    util::time::{DbTime, paris_day_start},
    web::coffres::{CoffreQuery, PAGE_SIZE},
};
use anyhow::Result;
use axum::extract::{Path, State};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};

/// Nom de propriété : comme `/coffre nommer`.
const PROPERTY_NAME_LIMIT: usize = 80;
/// Jours au plus dans une série (« tout »).
const SERIES_MAX_DAYS: i64 = 3650;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreProperty)]
pub struct Property {
    pub id: String,
    #[schema(required = true)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreItem)]
pub struct ItemChoice {
    pub item: String,
    pub label: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreLink)]
pub struct Link {
    pub game_id: String,
    pub name: String,
    pub member: MemberRef,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreChannel)]
pub struct ChannelInfo {
    pub channel_id: String,
    #[schema(required = true)]
    pub name: Option<String>,
    pub webhook_known: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffreMeta {
    pub properties: Vec<Property>,
    pub items: Vec<ItemChoice>,
    pub links: Vec<Link>,
    pub channels: Vec<ChannelInfo>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreStockItem)]
pub struct StockItem {
    pub item: String,
    pub label: String,
    pub deposited: i64,
    pub withdrawn: i64,
    /// Somme des corrections d'inventaire (signée).
    pub adjusted: i64,
    /// Dépôts − retraits + corrections.
    pub quantity: i64,
    pub low: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreStockProperty)]
pub struct StockProperty {
    pub id: String,
    #[schema(required = true)]
    pub name: Option<String>,
    pub items: Vec<StockItem>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffreStock {
    pub properties: Vec<StockProperty>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreMovementPlayer)]
pub struct Player {
    #[schema(required = true)]
    pub game_id: Option<String>,
    pub name: String,
    #[schema(required = true)]
    pub member: Option<MemberRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffreMovement {
    pub id: String,
    pub at: String,
    #[schema(value_type = super::openapi::CoffreKind)]
    pub kind: &'static str,
    pub property_id: String,
    #[schema(required = true)]
    pub property_name: Option<String>,
    pub item: String,
    pub item_label: String,
    /// Quantité ; signée pour une correction (`ADJUST`).
    pub amount: i64,
    /// Joueur du jeu ; pour une correction, le membre qui a saisi l'inventaire.
    pub player: Player,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffrePlayer {
    #[schema(required = true)]
    pub game_id: Option<String>,
    pub name: String,
    #[schema(required = true)]
    pub member: Option<MemberRef>,
    pub deposited: i64,
    pub withdrawn: i64,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreSeriesPoint)]
pub struct SeriesPoint {
    pub day: String,
    pub deposited: i64,
    pub withdrawn: i64,
    /// Corrections d'inventaire du jour (signées).
    pub adjusted: i64,
    pub stock: i64,
}

#[derive(Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub property_id: String,
    pub item: String,
    pub amount: i64,
    pub kind: String,
    pub player_name: String,
    pub player_game_id: Option<String>,
    pub at: DbTime,
    /// Auteur d'une correction d'inventaire.
    pub author_id: Option<String>,
}

/// Clause `WHERE` du filtre (mêmes règles que le module).
fn filter_sql(filter: &Filter) -> (String, Vec<String>) {
    let mut clauses = vec![r#""guildId" = ?"#.to_owned()];
    let mut binds = Vec::new();
    if let Some(property) = &filter.property_id {
        clauses.push(r#""propertyId" = ?"#.to_owned());
        binds.push(property.clone());
    }
    if let Some(item) = &filter.item {
        clauses.push(r#""item" = ?"#.to_owned());
        binds.push(item.clone());
    }
    if let Some(players) = &filter.player_game_ids {
        if players.is_empty() {
            clauses.push("0".to_owned());
        } else {
            clauses.push(format!(r#""playerGameId" IN ({})"#, vec!["?"; players.len()].join(", ")));
            binds.extend(players.iter().cloned());
        }
    }
    if let Some(kind) = filter.kind {
        clauses.push(r#""kind" = ?"#.to_owned());
        binds.push(kind.as_str().to_owned());
    }
    if let Some(since) = filter.since {
        clauses.push(r#""at" >= ?"#.to_owned());
        binds.push(since.to_db_string());
    }
    (clauses.join(" AND "), binds)
}

/// Mouvements filtrés, les plus récents d'abord, avec leur identifiant.
pub async fn movement_rows(
    db: &SqlitePool,
    guild_id: &str,
    filter: &Filter,
    limit: i64,
    offset: i64,
) -> Result<Vec<Row>> {
    let (clause, binds) = filter_sql(filter);
    let sql = format!(
        r#"SELECT "id", "propertyId", "item", "amount", "kind", "playerName", "playerGameId", "at", "authorId"
           FROM "CoffreMovement" WHERE {clause} ORDER BY "at" DESC, "id" DESC LIMIT ? OFFSET ?"#
    );
    let mut query = sqlx::query_as::<_, Row>(&sql).bind(guild_id);
    for bind in &binds {
        query = query.bind(bind);
    }
    Ok(query.bind(limit).bind(offset).fetch_all(db).await?)
}

/// Noms donnés aux propriétés.
pub async fn property_names(db: &SqlitePool, guild_id: &str) -> Result<HashMap<String, String>> {
    Ok(service::properties(db, guild_id).await?.into_iter().filter_map(|(id, name)| Some((id, name?))).collect())
}

pub fn movement(
    members: &Members,
    names: &HashMap<String, String>,
    links: &HashMap<String, String>,
    row: Row,
) -> CoffreMovement {
    let kind = Kind::parse(&row.kind).unwrap_or(Kind::Deposit);
    let member = match &row.author_id {
        Some(author) => Some(members.get_or(author, Some(&row.player_name))),
        None => row.player_game_id.as_ref().and_then(|id| links.get(id)).map(|user| members.get(user)),
    };
    CoffreMovement {
        id: row.id,
        at: iso_db(row.at),
        kind: kind.as_str(),
        property_name: names.get(&row.property_id).cloned(),
        property_id: row.property_id,
        item_label: item_label(&row.item),
        item: row.item,
        amount: row.amount,
        player: Player { member, game_id: row.player_game_id, name: row.player_name },
    }
}

/// Propriétés, objets, liens et salons suivis (`coffre.voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/meta", tag = "coffres", operation_id = "getCoffreMeta",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Méta-données", body = CoffreMeta),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn meta(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let gid = guild_id.to_string();
    let members = Members::new(&ctx, guild_id);
    let properties = service::properties(&ctx.db, &gid).await?;
    let items = service::items(&ctx.db, &gid).await?;
    // Nom du personnage : le dernier vu dans les mouvements.
    let links: Vec<(String, String, Option<String>)> = sqlx::query_as(
        r#"SELECT p."gameId", p."userId",
                  (SELECT m."playerName" FROM "CoffreMovement" m
                   WHERE m."guildId" = p."guildId" AND m."playerGameId" = p."gameId" ORDER BY m."at" DESC LIMIT 1)
           FROM "CoffrePlayer" p WHERE p."guildId" = ? ORDER BY p."gameId""#,
    )
    .bind(&gid)
    .fetch_all(&ctx.db)
    .await?;
    let channels = service::channels(&ctx.db, &gid).await?;
    Ok(json(&CoffreMeta {
        properties: properties.into_iter().map(|(id, name)| Property { id, name }).collect(),
        items: items.into_iter().map(|item| ItemChoice { label: item_label(&item), item }).collect(),
        links: links
            .into_iter()
            .map(|(game_id, user, name)| Link {
                name: name.unwrap_or_else(|| game_id.clone()),
                game_id,
                member: members.get(&user),
            })
            .collect(),
        channels: channels
            .into_iter()
            .map(|c| ChannelInfo {
                name: c
                    .channel_id
                    .parse()
                    .ok()
                    .and_then(twilight_model::id::Id::new_checked)
                    .and_then(|id| ctx.cache.channel(id))
                    .and_then(|channel| channel.name.clone()),
                webhook_known: c.webhook_id.is_some(),
                channel_id: c.channel_id,
            })
            .collect(),
    }))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StockQuery {
    /// Une seule propriété.
    propriete: Option<String>,
}

/// Stock par propriété (`coffre.voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/stock", tag = "coffres", operation_id = "getCoffreStock",
    params(("guild" = String, Path, description = "Identifiant du serveur"), StockQuery),
    responses(
        (status = 200, description = "Stock", body = CoffreStock),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn stock(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<StockQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let gid = guild_id.to_string();
    let property = query.propriete.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty());
    let names = property_names(&ctx.db, &gid).await?;
    let lines = service::stock(&ctx.db, &gid, property.as_deref()).await?;
    let thresholds = crate::modules::coffres::alerts::thresholds(&ctx.db, &gid).await?;
    let properties = group_stock(lines)
        .into_iter()
        .map(|(id, lines)| StockProperty {
            name: names.get(&id).cloned(),
            items: lines
                .into_iter()
                .map(|line| StockItem {
                    low: thresholds.is_low(&id, &line.item, line.quantity()),
                    label: item_label(&line.item),
                    quantity: line.quantity(),
                    item: line.item,
                    deposited: line.deposited,
                    withdrawn: line.withdrawn,
                    adjusted: line.adjusted,
                })
                .collect(),
            id,
        })
        .collect();
    Ok(json(&CoffreStock { properties }))
}

/// Mouvements filtrés (`coffre.voir`) ; `jours` absent = 30, `tout` = tout.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/movements", tag = "coffres", operation_id = "getCoffreMovements",
    params(("guild" = String, Path, description = "Identifiant du serveur"), CoffreQuery),
    responses(
        (status = 200, description = "Mouvements", body = super::openapi::CoffreMovementPage),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn movements(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<CoffreQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let gid = guild_id.to_string();
    let parsed = query.parse();
    let filter = parsed.filter();
    let total = service::count(&ctx.db, &gid, &filter).await?;
    let (page, offset) = clamp_page(parsed.page, total, PAGE_SIZE);
    let rows = movement_rows(&ctx.db, &gid, &filter, PAGE_SIZE, offset).await?;
    let names = property_names(&ctx.db, &gid).await?;
    let links = service::player_links(&ctx.db, &gid).await?;
    let members = Members::new(&ctx, guild_id);
    Ok(json(&Page {
        items: rows.into_iter().map(|row| movement(&members, &names, &links, row)).collect::<Vec<_>>(),
        total,
        page,
        page_size: PAGE_SIZE,
    }))
}

/// Activité par joueur, mêmes filtres que les mouvements (`page` ignoré).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/players", tag = "coffres", operation_id = "getCoffrePlayers",
    params(("guild" = String, Path, description = "Identifiant du serveur"), CoffreQuery),
    responses(
        (status = 200, description = "Joueurs", body = Vec<CoffrePlayer>),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn players(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<CoffreQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let gid = guild_id.to_string();
    let (clause, binds) = filter_sql(&query.parse().filter());
    let sql = format!(
        r#"SELECT MAX("playerGameId"), MAX("playerName"),
                  COALESCE(SUM(CASE WHEN "kind" = 'DEPOSIT' THEN "amount" END), 0),
                  COALESCE(SUM(CASE WHEN "kind" = 'WITHDRAW' THEN "amount" END), 0),
                  COUNT(*)
           FROM "CoffreMovement" WHERE {clause} AND "kind" IN ('DEPOSIT', 'WITHDRAW')
           GROUP BY COALESCE("playerGameId", "playerName") ORDER BY COUNT(*) DESC"#
    );
    let mut sql_query = sqlx::query_as::<_, (Option<String>, String, i64, i64, i64)>(&sql).bind(&gid);
    for bind in &binds {
        sql_query = sql_query.bind(bind);
    }
    let rows = sql_query.fetch_all(&ctx.db).await?;
    let links = service::player_links(&ctx.db, &gid).await?;
    let members = Members::new(&ctx, guild_id);
    let players: Vec<CoffrePlayer> = rows
        .into_iter()
        .map(|(game_id, name, deposited, withdrawn, count)| CoffrePlayer {
            member: game_id.as_ref().and_then(|id| links.get(id)).map(|user| members.get(user)),
            game_id,
            name,
            deposited,
            withdrawn,
            count,
        })
        .collect();
    Ok(json(&players))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SeriesQuery {
    /// Objet suivi (obligatoire).
    objet: Option<String>,
    /// Une seule propriété.
    propriete: Option<String>,
    /// Nombre de jours (30 par défaut), ou `tout`.
    jours: Option<String>,
}

/// Un point par jour : dépôts, retraits et stock cumulé (tout l'historique
/// avant la période compris). `days` : jours de Paris de la période.
pub async fn item_series(
    db: &SqlitePool,
    guild_id: &str,
    item: &str,
    property: Option<&str>,
    days: &[chrono::NaiveDate],
) -> Result<Vec<SeriesPoint>> {
    let Some(first) = days.first() else {
        return Ok(Vec::new());
    };
    let (before,): (i64,) = sqlx::query_as(
        r#"SELECT COALESCE(SUM(CASE "kind" WHEN 'WITHDRAW' THEN -"amount" ELSE "amount" END), 0)
           FROM "CoffreMovement" WHERE "guildId" = ? AND "item" = ? AND (? IS NULL OR "propertyId" = ?) AND "at" < ?"#,
    )
    .bind(guild_id)
    .bind(item)
    .bind(property)
    .bind(property)
    .bind(paris_day_start(*first))
    .fetch_one(db)
    .await?;
    let (cte, binds) = series::buckets(days, Duration::days(1));
    let sql = format!(
        r#"{cte} SELECT b."k",
                  COALESCE(SUM(CASE WHEN m."kind" = 'DEPOSIT' THEN m."amount" END), 0),
                  COALESCE(SUM(CASE WHEN m."kind" = 'WITHDRAW' THEN m."amount" END), 0),
                  COALESCE(SUM(CASE WHEN m."kind" = 'ADJUST' THEN m."amount" END), 0)
           FROM b LEFT JOIN "CoffreMovement" m ON m."guildId" = ? AND m."item" = ?
             AND (? IS NULL OR m."propertyId" = ?) AND m."at" >= b."s" AND m."at" < b."e"
           GROUP BY b."k" ORDER BY b."k""#
    );
    let mut query = sqlx::query_as::<_, (String, i64, i64, i64)>(&sql);
    for bind in binds {
        query = query.bind(bind);
    }
    let rows = query.bind(guild_id).bind(item).bind(property).bind(property).fetch_all(db).await?;
    let mut stock = before;
    Ok(rows
        .into_iter()
        .map(|(day, deposited, withdrawn, adjusted)| {
            stock += deposited - withdrawn + adjusted;
            SeriesPoint { day, deposited, withdrawn, adjusted, stock }
        })
        .collect())
}

/// Évolution d'un objet, un point par jour (`coffre.voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/series", tag = "coffres", operation_id = "getCoffreSeries",
    params(("guild" = String, Path, description = "Identifiant du serveur"), SeriesQuery),
    responses(
        (status = 200, description = "Série", body = Vec<SeriesPoint>),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn series(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<SeriesQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let gid = guild_id.to_string();
    let item = query.objet.map(|o| o.trim().to_owned()).filter(|o| !o.is_empty());
    let Some(item) = item else {
        return Err(ApiError::bad_request("Indique l'objet (paramètre « objet »)."));
    };
    let property = query.propriete.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty());
    let today = paris_date(Utc::now());
    // Même période que l'historique : 30 jours par défaut, « tout » depuis le premier mouvement.
    let first = match query.jours.as_deref().map(str::trim) {
        Some("tout") => {
            let (first,): (Option<DbTime>,) = sqlx::query_as(
                r#"SELECT MIN("at") FROM "CoffreMovement" WHERE "guildId" = ? AND "item" = ? AND (? IS NULL OR "propertyId" = ?)"#,
            )
            .bind(&gid)
            .bind(&item)
            .bind(&property)
            .bind(&property)
            .fetch_one(&ctx.db)
            .await?;
            let first = first.map_or(today, |at| paris_date(at.0));
            first.max(today - Duration::days(SERIES_MAX_DAYS - 1))
        }
        raw => {
            let days =
                raw.and_then(|d| d.parse::<i64>().ok()).filter(|d| (1..=SERIES_MAX_DAYS).contains(d)).unwrap_or(30);
            today - Duration::days(days - 1)
        }
    };
    let days = series::days(first, today);
    Ok(json(&item_series(&ctx.db, &gid, &item, property.as_deref(), &days).await?))
}

/// Export CSV, mêmes filtres que les mouvements.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/export.csv", tag = "coffres", operation_id = "exportCoffres",
    params(("guild" = String, Path, description = "Identifiant du serveur"), CoffreQuery),
    responses(
        (status = 200, description = "Fichier CSV", content_type = "text/csv", body = String),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn export(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<CoffreQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    Ok(crate::web::coffres::csv_response(&ctx, guild_id, &query).await)
}

/// Nom vide (ou `null`) : retirer le nom.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PropertyName {
    #[schema(required = true)]
    name: Option<String>,
}

/// Nomme une propriété, nom vide : retire le nom (`coffre.gerer`).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/coffres/properties/{property}", tag = "coffres", operation_id = "nameCoffreProperty",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("property" = String, Path, description = "Identifiant de la propriété")),
    request_body = PropertyName,
    responses(
        (status = 200, description = "Propriété", body = Property),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn name_property(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, property)): Path<(String, String)>,
    JsonBody(input): JsonBody<PropertyName>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreGerer)?;
    let gid = guild_id.to_string();
    let property = property.trim().to_owned();
    if property.is_empty() || property.chars().count() > 100 {
        return Err(ApiError::invalid("Propriété invalide."));
    }
    let name = input.name.unwrap_or_default().trim().to_owned();
    if name.chars().count() > PROPERTY_NAME_LIMIT {
        return Err(ApiError::invalid(format!("Le nom ne doit pas dépasser {PROPERTY_NAME_LIMIT} caractères.")));
    }
    let details = if name.is_empty() {
        sqlx::query(r#"DELETE FROM "CoffreProperty" WHERE "guildId" = ? AND "propertyId" = ?"#)
            .bind(&gid)
            .bind(&property)
            .execute(&ctx.db)
            .await?;
        format!("Propriété `{property}` : nom retiré")
    } else {
        service::name_property(&ctx.db, &gid, &property, &name).await?;
        format!("Propriété `{property}` : **{name}**")
    };
    let actor = session.actor_name(&ctx, guild_id);
    audit::record_web(&ctx, guild_id, session.viewer.id, &actor, Entry::new(Action::CoffreName, None, details)).await;
    Ok(json(&Property { id: property, name: Some(name).filter(|n| !n.is_empty()) }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreInventoryLine)]
pub struct InventoryLine {
    /// Identifiant de l'objet dans le jeu, ex : `repair_kit`.
    item: String,
    /// Quantité réellement présente dans le coffre.
    quantity: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreInventory)]
pub struct Inventory {
    property_id: String,
    items: Vec<InventoryLine>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreCorrection)]
pub struct CorrectionView {
    pub item: String,
    pub label: String,
    pub before: i64,
    pub after: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CoffreInventoryResult)]
pub struct InventoryResult {
    /// Seuls les objets dont la quantité a changé.
    pub corrections: Vec<CorrectionView>,
}

/// Valide un inventaire : propriété en chiffres, au moins un objet, 300 au plus,
/// identifiants d'objets du jeu, quantités entre 0 et le maximum.
fn check_inventory(input: Inventory) -> Result<(String, Vec<(String, i64)>), String> {
    let property = input.property_id.trim().to_owned();
    if property.is_empty() || property.len() > 30 || !property.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Propriété invalide : son numéro, ex : 8740.".to_owned());
    }
    if input.items.is_empty() || input.items.len() > 300 {
        return Err("Entre 1 et 300 objets par inventaire.".to_owned());
    }
    let mut counted: Vec<(String, i64)> = Vec::new();
    for line in input.items {
        let item = service::normalize_item(&line.item).ok_or_else(|| format!("Objet invalide : « {} ».", line.item))?;
        if !(0..=service::MAX_INVENTORY).contains(&line.quantity) {
            return Err(format!("{} : quantité entre 0 et {}.", item_label(&item), service::MAX_INVENTORY));
        }
        match counted.iter_mut().find(|(known, _)| *known == item) {
            Some(entry) => entry.1 = line.quantity,
            None => counted.push((item, line.quantity)),
        }
    }
    Ok((property, counted))
}

/// Inventaire d'une propriété, comme `/coffre inventaire` (`coffre.gerer`) : les
/// écarts avec le stock connu deviennent des corrections signées, journalisées.
#[utoipa::path(
    put, path = "/api/guilds/{guild}/coffres/inventory", tag = "coffres", operation_id = "setCoffreInventory",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = Inventory,
    responses(
        (status = 200, description = "Corrections enregistrées", body = InventoryResult),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn inventory(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(input): JsonBody<Inventory>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreGerer)?;
    let gid = guild_id.to_string();
    let (property, counted) = check_inventory(input).map_err(ApiError::invalid)?;
    let actor = session.actor_name(&ctx, guild_id);
    let corrections =
        service::set_inventory(&ctx.db, &gid, &property, &counted, &session.viewer.id.to_string(), &actor).await?;
    if !corrections.is_empty() {
        let names = property_names(&ctx.db, &gid).await?;
        let label = names.get(&property).cloned().unwrap_or_else(|| format!("Propriété {property}"));
        let lines: Vec<String> = corrections
            .iter()
            .map(|c| format!("**{}** : {} → {} ({:+})", item_label(&c.item), c.before, c.after, c.delta()))
            .collect();
        let entry = Entry::new(Action::CoffreInventory, None, format!("**{label}** — {}", lines.join(", ")));
        audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
        crate::modules::coffres::dashboard::request_refresh(&ctx, &gid);
    }
    Ok(json(&InventoryResult {
        corrections: corrections
            .into_iter()
            .map(|c| CorrectionView { label: item_label(&c.item), item: c.item, before: c.before, after: c.after })
            .collect(),
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerLink {
    member_id: String,
}

fn check_game_id(raw: &str) -> ApiResult<String> {
    let game_id = raw.trim().to_owned();
    if game_id.is_empty() || game_id.len() > 30 || !game_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ApiError::invalid("Identifiant invalide : uniquement des chiffres, ex : 340772."));
    }
    Ok(game_id)
}

/// Relie un personnage du jeu à un membre, comme `/coffre lier` (`coffre.gerer`).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/coffres/players/{game_id}", tag = "coffres", operation_id = "linkCoffrePlayer",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("game_id" = String, Path, description = "Identifiant du personnage dans le jeu")),
    request_body = PlayerLink,
    responses(
        (status = 200, description = "Lien", body = Link),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn link_player(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, game_id)): Path<(String, String)>,
    JsonBody(input): JsonBody<PlayerLink>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreGerer)?;
    let gid = guild_id.to_string();
    let game_id = check_game_id(&game_id)?;
    let member = parse_user(&input.member_id)?;
    super::require_member(&ctx, guild_id, member).await?;
    service::link_player(&ctx.db, &gid, &game_id, &member.to_string()).await?;
    let actor = session.actor_name(&ctx, guild_id);
    let entry = Entry::new(Action::CoffreLink, Some(member.to_string()), format!("Personnage `{game_id}`"));
    audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    let name: Option<(String,)> = sqlx::query_as(
        r#"SELECT "playerName" FROM "CoffreMovement" WHERE "guildId" = ? AND "playerGameId" = ? ORDER BY "at" DESC LIMIT 1"#,
    )
    .bind(&gid)
    .bind(&game_id)
    .fetch_optional(&ctx.db)
    .await?;
    Ok(json(&Link {
        name: name.map_or_else(|| game_id.clone(), |(n,)| n),
        member: Members::new(&ctx, guild_id).get(&member.to_string()),
        game_id,
    }))
}

/// Retire le lien d'un personnage (`coffre.gerer`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/coffres/players/{game_id}", tag = "coffres", operation_id = "unlinkCoffrePlayer",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("game_id" = String, Path, description = "Identifiant du personnage dans le jeu")),
    responses(
        (status = 204, description = "Lien retiré"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn unlink_player(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, game_id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreGerer)?;
    let gid = guild_id.to_string();
    let game_id = check_game_id(&game_id)?;
    let removed: Option<(String,)> =
        sqlx::query_as(r#"DELETE FROM "CoffrePlayer" WHERE "guildId" = ? AND "gameId" = ? RETURNING "userId""#)
            .bind(&gid)
            .bind(&game_id)
            .fetch_optional(&ctx.db)
            .await?;
    let Some((user,)) = removed else {
        return Err(ApiError::not_found("Ce personnage n'est relié à aucun membre."));
    };
    let actor = session.actor_name(&ctx, guild_id);
    let entry = Entry::new(Action::CoffreUnlink, Some(user), format!("Personnage `{game_id}`"));
    audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    Ok(no_content())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::coffres::{parser::Movement, service::NewMovement};
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn validates_inventories() {
        let input = |property: &str, items: serde_json::Value| -> Inventory {
            serde_json::from_value(serde_json::json!({ "propertyId": property, "items": items })).unwrap()
        };
        let (property, counted) = check_inventory(input(
            " 8740 ",
            serde_json::json!([{ "item": "Repair Kit", "quantity": 4 }, { "item": "repair_kit", "quantity": 5 }]),
        ))
        .unwrap();
        assert_eq!((property.as_str(), counted), ("8740", vec![("repair_kit".to_owned(), 5)]));
        assert!(check_inventory(input("abc", serde_json::json!([{ "item": "kit", "quantity": 1 }]))).is_err());
        assert!(check_inventory(input("1", serde_json::json!([]))).is_err());
        assert!(check_inventory(input("1", serde_json::json!([{ "item": "kit", "quantity": -1 }]))).is_err());
        assert!(check_inventory(input("1", serde_json::json!([{ "item": "<script>", "quantity": 1 }]))).is_err());
    }

    #[tokio::test]
    async fn series_fill_gaps_and_carry_stock() {
        let dir = std::env::temp_dir().join(format!("api-coffre-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let movement = |id: &str, amount, kind, day: u32, hour: u32| NewMovement {
            id: id.into(),
            movement: Movement {
                source: "Coffre propriété".into(),
                property_id: "8740".into(),
                chest: Some(1),
                item: "repair_kit".into(),
                amount,
                kind,
                player_name: "Kyle".into(),
                player_game_id: Some("1".into()),
                at: None,
            },
            at: DbTime(Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()),
        };
        let rows = [
            movement("a", 10, Kind::Deposit, 1, 12), // avant la période
            movement("b", 3, Kind::Withdraw, 5, 12), // 05/10
            movement("c", 4, Kind::Deposit, 7, 21),  // 07/10 23:00 à Paris
            movement("d", 2, Kind::Deposit, 7, 22),  // 08/10 00:00 à Paris
        ];
        service::add_channel(&db, "g", "c").await.unwrap();
        service::insert_all(&db, "g", "c", &rows, None).await.unwrap();
        let day = |d| NaiveDate::from_ymd_opt(2026, 10, d).unwrap();
        let days = series::days(day(5), day(8));
        let points = item_series(&db, "g", "repair_kit", None, &days).await.unwrap();
        let summary: Vec<(&str, i64, i64, i64)> =
            points.iter().map(|p| (p.day.as_str(), p.deposited, p.withdrawn, p.stock)).collect();
        assert_eq!(
            summary,
            [("2026-10-05", 0, 3, 7), ("2026-10-06", 0, 0, 7), ("2026-10-07", 4, 0, 11), ("2026-10-08", 2, 0, 13)]
        );
        assert!(item_series(&db, "g", "repair_kit", Some("autre"), &days).await.unwrap().iter().all(|p| p.stock == 0));
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
