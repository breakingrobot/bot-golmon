//! Cotisations : statistiques, déclaration et suppression (comme `/cotisation`).

use super::{
    ApiError, ApiQuery, ApiResult, JsonBody, MemberRef, Members, Page, Session, clamp_page, created, json, no_content,
    paris_day,
    series::{self, paris_date},
};
use crate::{
    context::{Context, Ctx},
    discord::{
        embed::{EmbedBuilder, EmbedBuilderExt as _, footer},
        members::avatar_url,
        messages::{self, Outgoing, parse_id},
    },
    modules::{
        audit::{self, Action, Entry},
        cotisations::{
            money,
            service::{self, Cotisation, NewCotisation as NewRow, Totals},
        },
        permissions::Perm,
    },
    util::{
        format::{field, truncate_field},
        time::{DbTime, format_day_label, paris_day_start, parse_iso_day},
    },
    web::cotisations::{CotisationQuery, PAGE_SIZE},
};
use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::Response,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::ToSchema;

/// Montant maximal par cotisation (propre ou sale), comme `/cotisation ajouter`.
pub const MAX_AMOUNT: i64 = 100_000_000;
pub const DESCRIPTION_LIMIT: usize = 300;
const ANNOUNCE_COLOR: u32 = 0x2ecc71;
/// Semaines affichées pour « tout ».
const DEFAULT_WEEKS: i64 = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CotisationTotals {
    pub count: i64,
    pub propre: i64,
    pub sale: i64,
}

impl From<Totals> for CotisationTotals {
    fn from(totals: Totals) -> Self {
        Self { count: totals.count, propre: totals.propre, sale: totals.sale }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAmount {
    pub week_start: String,
    pub propre: i64,
    pub sale: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CotisationEntry {
    pub id: String,
    pub date: String,
    pub member: MemberRef,
    pub propre: i64,
    pub sale: i64,
    pub description: String,
    #[schema(required = true)]
    pub declared_by: Option<MemberRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = CotisationMember)]
pub struct MemberTotals {
    pub member: MemberRef,
    pub propre: i64,
    pub sale: i64,
    pub count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CotisationsResponse {
    pub totals: CotisationTotals,
    pub members: Vec<MemberTotals>,
    pub weekly: Vec<WeeklyAmount>,
    #[schema(value_type = super::openapi::CotisationEntryPage)]
    pub entries: Page<CotisationEntry>,
    #[schema(required = true)]
    pub objective: Option<super::openapi::Objective>,
    pub objective_progress: Vec<super::openapi::ObjectiveProgress>,
}

pub fn entry(members: &Members, row: &Cotisation) -> CotisationEntry {
    CotisationEntry {
        id: row.id.clone(),
        date: paris_day(row.date),
        member: members.get(&row.user_id),
        propre: row.propre,
        sale: row.sale,
        description: row.description.clone(),
        declared_by: (row.created_by_id != row.user_id).then(|| members.get(&row.created_by_id)),
    }
}

/// Montants par semaine (`weeks` : lundis), semaines vides comprises.
pub async fn weekly(
    db: &SqlitePool,
    guild_id: &str,
    user_id: Option<&str>,
    since: Option<DbTime>,
    weeks: &[NaiveDate],
) -> Result<Vec<WeeklyAmount>> {
    if weeks.is_empty() {
        return Ok(Vec::new());
    }
    let (cte, binds) = series::buckets(weeks, Duration::weeks(1));
    let sql = format!(
        r#"{cte} SELECT b."k", COALESCE(SUM(c."propre"), 0), COALESCE(SUM(c."sale"), 0) FROM b
           LEFT JOIN "Cotisation" c ON c."guildId" = ? AND c."date" >= b."s" AND c."date" < b."e"
             AND (? IS NULL OR c."userId" = ?) AND (? IS NULL OR c."date" >= ?)
           GROUP BY b."k" ORDER BY b."k""#
    );
    let mut query = sqlx::query_as::<_, (String, i64, i64)>(&sql);
    for bind in binds {
        query = query.bind(bind);
    }
    let since = since.map(DbTime::to_db_string);
    let rows = query.bind(guild_id).bind(user_id).bind(user_id).bind(&since).bind(&since).fetch_all(db).await?;
    Ok(rows.into_iter().map(|(week_start, propre, sale)| WeeklyAmount { week_start, propre, sale }).collect())
}

/// Statistiques et détail (`cotisation.stats`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/cotisations", tag = "cotisations", operation_id = "getCotisations",
    params(("guild" = String, Path, description = "Identifiant du serveur"), CotisationQuery),
    responses(
        (status = 200, description = "Cotisations", body = CotisationsResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn list(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<CotisationQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CotisationStats)?;
    let gid = guild_id.to_string();
    let parsed = query.parse();
    let filter = parsed.filter();
    let members = Members::new(&ctx, guild_id);

    let totals = service::totals(&ctx.db, &gid, &filter).await?;
    let by_member = service::by_member(&ctx.db, &gid, &filter).await?;
    let (page, offset) = clamp_page(parsed.page, totals.count, PAGE_SIZE);
    let rows = service::list(&ctx.db, &gid, &filter, PAGE_SIZE, offset).await?;

    let today = paris_date(Utc::now());
    let weeks = match filter.since {
        Some(since) => series::weeks(paris_date(since.0), today),
        None => series::last_weeks(today, DEFAULT_WEEKS),
    };
    let weekly = weekly(&ctx.db, &gid, parsed.member.as_deref(), filter.since, &weeks).await?;

    Ok(json(&CotisationsResponse {
        members: by_member
            .into_iter()
            .map(|(user, t)| MemberTotals {
                member: members.get(&user),
                propre: t.propre,
                sale: t.sale,
                count: t.count,
            })
            .collect(),
        weekly,
        entries: Page {
            items: rows.iter().map(|row| entry(&members, row)).collect(),
            total: totals.count,
            page,
            page_size: PAGE_SIZE,
        },
        totals: totals.into(),
        objective: super::reglages::current_objective(&ctx, &guild_id.to_string()).await?,
        objective_progress: super::reglages::week_progress(&ctx, guild_id).await?,
    }))
}

/// Export CSV, mêmes filtres que la liste.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/cotisations/export.csv", tag = "cotisations", operation_id = "exportCotisations",
    params(("guild" = String, Path, description = "Identifiant du serveur"), CotisationQuery),
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
    ApiQuery(query): ApiQuery<CotisationQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CotisationStats)?;
    Ok(crate::web::cotisations::csv_response(&ctx, guild_id, &query).await)
}

/// Absent : soi-même (`memberId`) et aujourd'hui (`date`, jamais dans le futur).
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct NewCotisation {
    pub member_id: Option<String>,
    #[schema(required = true)]
    pub propre: i64,
    #[schema(required = true)]
    pub sale: i64,
    #[schema(required = true)]
    pub description: String,
    pub date: Option<String>,
}

/// Saisie validée : montants, description et jour (minuit de Paris).
#[derive(Debug, PartialEq, Eq)]
pub struct Checked {
    pub propre: i64,
    pub sale: i64,
    pub description: String,
    pub date: DbTime,
}

/// Mêmes règles que `/cotisation ajouter`.
pub fn validate(input: &NewCotisation, now: DbTime) -> Result<Checked, String> {
    let (propre, sale) = (input.propre, input.sale);
    if !(0..=MAX_AMOUNT).contains(&propre) || !(0..=MAX_AMOUNT).contains(&sale) {
        return Err(format!("Montant invalide : entre 0 et {} par cotisation.", money(MAX_AMOUNT)));
    }
    if propre + sale <= 0 {
        return Err("Indique un montant en argent propre et/ou sale.".to_owned());
    }
    let description = input.description.trim().to_owned();
    if description.is_empty() {
        return Err("Indique d'où vient l'argent (description).".to_owned());
    }
    if description.chars().count() > DESCRIPTION_LIMIT {
        return Err(format!("La description ne doit pas dépasser {DESCRIPTION_LIMIT} caractères."));
    }
    let day = match input.date.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(raw) => parse_iso_day(raw).ok_or("Date invalide. Format attendu : AAAA-MM-JJ.")?,
        None => paris_date(now.0),
    };
    let date = paris_day_start(day);
    if date > now {
        return Err("La date ne peut pas être dans le futur.".to_owned());
    }
    Ok(Checked { propre, sale, description, date })
}

/// « 1 000 $ propre · 500 $ sale ».
fn amounts(propre: i64, sale: i64) -> String {
    match (propre, sale) {
        (0, sale) => format!("{} sale", money(sale)),
        (propre, 0) => format!("{} propre", money(propre)),
        (propre, sale) => format!("{} propre · {} sale", money(propre), money(sale)),
    }
}

/// Annonce dans le salon des cotisations, comme `/cotisation ajouter`.
async fn announce(ctx: &Ctx, guild: &str, row: &Cotisation, declared_by: Option<&str>) {
    let channel = match service::channel(&ctx.db, guild).await {
        Ok(channel) => channel,
        Err(error) => return tracing::warn!(?error, "salon des cotisations illisible"),
    };
    let Some(channel_id) = channel.as_deref().and_then(parse_id) else {
        return;
    };
    let thumbnail = parse_id(&row.user_id).and_then(|id| ctx.cache.user(id)).map(|u| avatar_url(&u));
    let mut embed = EmbedBuilder::new()
        .title("💰 Nouvelle cotisation")
        .color(ANNOUNCE_COLOR)
        .field(field("Membre", format!("<@{}>", row.user_id), true))
        .field(field("Date", format_day_label(row.date), true))
        .field(field("Montant", amounts(row.propre, row.sale), false))
        .field(field("Description", truncate_field(&row.description), false))
        .maybe_thumbnail(thumbnail);
    if let Some(name) = declared_by {
        embed = embed.footer(footer(format!("Déclarée par {name}")));
    }
    if let Err(error) = messages::send(ctx, channel_id, &Outgoing::embeds(vec![embed.build()])).await {
        tracing::warn!(?error, "annonce de cotisation impossible");
    }
}

/// Déclare une cotisation (`cotisation.ajouter`, pour un autre : `cotisation.gerer`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/cotisations", tag = "cotisations", operation_id = "createCotisation",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = NewCotisation,
    responses(
        (status = 201, description = "Cotisation enregistrée", body = CotisationEntry),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn create(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(input): JsonBody<NewCotisation>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let target = match input.member_id.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        Some(raw) => super::parse_user(raw)?,
        None => session.viewer.id,
    };
    let for_someone_else = target != session.viewer.id;
    let perm = if for_someone_else { Perm::CotisationGerer } else { Perm::CotisationAjouter };
    if !session.can(guild_id, perm) {
        return Err(session.denied(guild_id, perm));
    }
    super::require_member(&ctx, guild_id, target).await?;
    let checked = validate(&input, DbTime::now()).map_err(ApiError::invalid)?;
    let gid = guild_id.to_string();
    let row = service::add(
        &ctx.db,
        &gid,
        NewRow {
            user_id: target.to_string(),
            propre: checked.propre,
            sale: checked.sale,
            description: checked.description.clone(),
            date: checked.date,
            created_by_id: session.viewer.id.to_string(),
        },
    )
    .await?;

    let actor = session.actor_name(&ctx, guild_id);
    announce(&ctx, &gid, &row, for_someone_else.then_some(actor.as_str())).await;
    if for_someone_else {
        let details = format!("{} — {}", amounts(row.propre, row.sale), row.description);
        let entry = Entry::new(Action::CotisationAddFor, Some(target.to_string()), details);
        audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    }
    Ok(created(&entry(&Members::new(&ctx, guild_id), &row)))
}

/// Supprime une cotisation (son auteur, ou `cotisation.gerer`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/cotisations/{id}", tag = "cotisations", operation_id = "deleteCotisation",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la cotisation")),
    responses(
        (status = 204, description = "Supprimée"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn remove(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult<Response> {
    let guild_id: Id<GuildMarker> = session.member_of(&guild)?;
    let gid = guild_id.to_string();
    let Some(row) = service::get(&ctx.db, &gid, id.trim()).await? else {
        return Err(ApiError::not_found("Cotisation introuvable."));
    };
    // Comme sur Discord : l'auteur de la déclaration, ou le staff.
    let own = row.created_by_id == session.viewer.id.to_string();
    if !own && !session.can(guild_id, Perm::CotisationGerer) {
        return Err(ApiError::forbidden("Tu ne peux supprimer que les cotisations que tu as déclarées toi-même."));
    }
    service::delete(&ctx.db, &gid, &row.id).await?;
    let details = format!("{} — {} — {}", format_day_label(row.date), amounts(row.propre, row.sale), row.description);
    let actor = session.actor_name(&ctx, guild_id);
    audit::record_web(
        &ctx,
        guild_id,
        session.viewer.id,
        &actor,
        Entry::new(Action::CotisationDelete, Some(row.user_id.clone()), details),
    )
    .await;
    Ok(no_content())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(propre: i64, sale: i64, description: &str, date: Option<&str>) -> NewCotisation {
        NewCotisation { member_id: None, propre, sale, description: description.into(), date: date.map(Into::into) }
    }

    #[test]
    fn validates_like_the_command() {
        let now = DbTime(chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 10, 19, 12, 0, 0).unwrap());
        let ok = validate(&input(1500, 0, " Vente ", Some("2026-10-19")), now).unwrap();
        assert_eq!((ok.propre, ok.description.as_str()), (1500, "Vente"));
        assert_eq!(ok.date.to_db_string(), "2026-10-18T22:00:00.000Z");
        assert_eq!(validate(&input(10, 0, "x", None), now).unwrap().date, ok.date, "aujourd'hui par défaut");
        assert!(validate(&input(0, 0, "x", None), now).unwrap_err().contains("montant"));
        assert!(validate(&input(-1, 10, "x", None), now).is_err());
        assert!(validate(&input(MAX_AMOUNT + 1, 0, "x", None), now).is_err());
        assert!(validate(&input(MAX_AMOUNT, MAX_AMOUNT, "x", None), now).is_ok());
        assert!(validate(&input(10, 0, "  ", None), now).is_err());
        assert!(validate(&input(10, 0, &"é".repeat(301), None), now).is_err());
        assert!(validate(&input(10, 0, "x", Some("2026-10-20")), now).unwrap_err().contains("futur"));
        assert!(validate(&input(10, 0, "x", Some("19/10/2026")), now).is_err());
    }
}
