//! Membres : recherche (sélecteurs) et fiche d'un membre.

use super::{
    ApiQuery, ApiResult, MemberRef, Members, RoleRef, Session, coffres, cotisations, iso, iso_db, json, parse_user,
    role_ref,
    series::{self, paris_date},
    stats::Veto,
};
use crate::{
    context::{Context, Ctx},
    modules::{
        coffres::service as coffres_service,
        cotisations::service::{self as cotisations_service, Filter},
        permissions::Perm,
        presences::service as presences,
    },
    util::time::DbTime,
};
use anyhow::Result;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use utoipa::{IntoParams, ToSchema};

const SEARCH_LIMIT: usize = 25;
const RECENT: i64 = 10;
const PROFILE_WEEKS: i64 = 12;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// Texte cherché dans le nom ou l'identifiant.
    q: Option<String>,
}

/// Membres en cache dont le nom ou l'identifiant contient `q` (25 au plus).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/members", tag = "membres", operation_id = "searchMembers",
    params(("guild" = String, Path, description = "Identifiant du serveur"), SearchQuery),
    responses(
        (status = 200, description = "Membres", body = Vec<MemberRef>),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn search(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<SearchQuery>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    Ok(json(&search_members(&ctx, guild_id, query.q.as_deref().unwrap_or_default())))
}

pub fn search_members(ctx: &Ctx, guild_id: Id<GuildMarker>, query: &str) -> Vec<MemberRef> {
    let query = query.trim().to_lowercase();
    let Some(ids) = ctx.cache.guild_members(guild_id) else {
        return Vec::new();
    };
    let lookup = Members::new(ctx, guild_id);
    let mut found: Vec<MemberRef> = ids
        .iter()
        .filter_map(|user_id| {
            let user = ctx.cache.user(*user_id);
            if user.as_ref().is_some_and(|u| u.bot) {
                return None;
            }
            let member = lookup.get(&user_id.to_string());
            let matches = query.is_empty()
                || member.name.to_lowercase().contains(&query)
                || member.id == query
                || user.as_ref().is_some_and(|u| {
                    u.name.to_lowercase().contains(&query)
                        || u.global_name.as_ref().is_some_and(|g| g.to_lowercase().contains(&query))
                });
            matches.then_some(member)
        })
        .collect();
    found.sort_by_key(|m| m.name.to_lowercase());
    found.truncate(SEARCH_LIMIT);
    found
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberRecentPresence)]
pub struct RecentPresence {
    pub id: String,
    pub title: String,
    pub date: String,
    #[schema(value_type = super::openapi::PresenceStatus)]
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberPresence)]
pub struct PresenceSection {
    pub present: i64,
    pub retard: i64,
    pub absent: i64,
    #[schema(required = true)]
    pub rate: Option<i64>,
    pub recent: Vec<RecentPresence>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberAbsence)]
pub struct AbsenceItem {
    pub from: String,
    pub to: String,
    pub motif: String,
    pub current: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberCotisations)]
pub struct CotisationSection {
    pub totals: cotisations::CotisationTotals,
    pub weekly: Vec<cotisations::WeeklyAmount>,
    pub recent: Vec<cotisations::CotisationEntry>,
    // TODO(intégration) : progression vers l'objectif de la semaine (autre chantier).
    #[schema(required = true)]
    pub objective: Option<super::openapi::ObjectiveProgress>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberCharacter)]
pub struct Character {
    pub game_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberCoffres)]
pub struct CoffreSection {
    pub characters: Vec<Character>,
    pub deposited: i64,
    pub withdrawn: i64,
    pub recent: Vec<coffres::CoffreMovement>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberRecrueWeek)]
pub struct RecrueWeek {
    pub week_start: String,
    pub pour: i64,
    pub contre: i64,
    pub blanc: i64,
    #[schema(required = true)]
    pub veto: Option<Veto>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberRecrue)]
pub struct RecrueSection {
    pub weeks: Vec<RecrueWeek>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = MemberVehicle)]
pub struct Vehicle {
    pub plate: String,
    #[schema(required = true)]
    pub model: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberProfile {
    pub member: MemberRef,
    #[schema(required = true)]
    pub joined_at: Option<String>,
    pub roles: Vec<RoleRef>,
    #[schema(required = true)]
    pub presence: Option<PresenceSection>,
    pub absences: Vec<AbsenceItem>,
    #[schema(required = true)]
    pub cotisations: Option<CotisationSection>,
    #[schema(required = true)]
    pub coffres: Option<CoffreSection>,
    #[schema(required = true)]
    pub recrue: Option<RecrueSection>,
    #[schema(required = true)]
    pub phone: Option<String>,
    pub vehicles: Vec<Vehicle>,
}

/// Fiche d'un membre : `membre.fiche`, ou sa propre fiche.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/members/{user}", tag = "membres", operation_id = "getMemberProfile",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("user" = String, Path, description = "Identifiant Discord du membre")),
    responses(
        (status = 200, description = "Fiche", body = MemberProfile),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn profile(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, user)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let user_id = parse_user(&user).map_err(|_| super::ApiError::not_found("Membre introuvable."))?;
    if user_id != session.viewer.id && !session.can(guild_id, Perm::MembreFiche) {
        return Err(session.denied(guild_id, Perm::MembreFiche));
    }
    // Avis et vétos : réservés au suivi des recrues, même sur sa propre fiche.
    let with_recrue = session.can(guild_id, Perm::RecrueSuivi);
    Ok(json(&build_profile(&ctx, guild_id, user_id, with_recrue).await?))
}

pub async fn build_profile(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    with_recrue: bool,
) -> Result<MemberProfile> {
    let guild = guild_id.to_string();
    let user = user_id.to_string();
    let lookup = Members::new(ctx, guild_id);
    let cached = ctx.cache.member(guild_id, user_id);
    let joined_at = cached
        .as_ref()
        .and_then(|m| m.joined_at())
        .and_then(|joined| DateTime::<Utc>::from_timestamp_micros(joined.as_micros()))
        .map(iso);
    let mut roles: Vec<(i64, RoleRef)> = cached
        .as_ref()
        .map(|m| m.roles().to_vec())
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let position = ctx.cache.role(id).map_or(0, |r| r.resource().position);
            (position, role_ref(ctx, &id.to_string()))
        })
        .collect();
    drop(cached);
    roles.sort_by_key(|role| std::cmp::Reverse(role.0));

    Ok(MemberProfile {
        member: lookup.get(&user),
        joined_at,
        roles: roles.into_iter().map(|(_, role)| role).collect(),
        presence: presence(&ctx.db, &guild, &user).await?,
        absences: absences(&ctx.db, &guild, &user).await?,
        cotisations: cotisation_section(ctx, guild_id, &user).await?,
        coffres: coffre_section(ctx, guild_id, &user).await?,
        recrue: if with_recrue { recrue(&ctx.db, &lookup, &guild, &user).await? } else { None },
        phone: sqlx::query_as::<_, (String,)>(
            r#"SELECT "numero" FROM "PhoneNumber" WHERE "guildId" = ? AND "userId" = ?"#,
        )
        .bind(&guild)
        .bind(&user)
        .fetch_optional(&ctx.db)
        .await?
        .map(|(numero,)| numero),
        vehicles: sqlx::query_as::<_, (String, Option<String>)>(
            r#"SELECT "plate", "modele" FROM "Vehicle" WHERE "guildId" = ? AND "ownerId" = ? AND NOT "ownerIsRole"
               ORDER BY "plate""#,
        )
        .bind(&guild)
        .bind(&user)
        .fetch_all(&ctx.db)
        .await?
        .into_iter()
        .map(|(plate, model)| Vehicle { plate, model })
        .collect(),
    })
}

#[derive(FromRow)]
#[sqlx(rename_all = "camelCase")]
struct PresenceRow {
    id: String,
    titre: String,
    event_date: DbTime,
    status: Option<String>,
}

/// `null` tant qu'aucune présence n'a été clôturée sur le serveur.
async fn presence(db: &SqlitePool, guild: &str, user: &str) -> Result<Option<PresenceSection>> {
    let stats = presences::user_stats(db, guild, user).await?;
    if stats.total_closed == 0 {
        return Ok(None);
    }
    let rows: Vec<PresenceRow> = sqlx::query_as(
        r#"SELECT p."id", p."titre", p."eventDate", r."status" FROM "Presence" p
           LEFT JOIN "PresenceResponse" r ON r."presenceId" = p."id" AND r."userId" = ?
           WHERE p."guildId" = ? AND p."closedAt" IS NOT NULL ORDER BY p."eventDate" DESC LIMIT ?"#,
    )
    .bind(user)
    .bind(guild)
    .bind(RECENT)
    .fetch_all(db)
    .await?;
    let rate = ((stats.present + stats.retard) as f64 * 100.0 / stats.total_closed as f64).round() as i64;
    Ok(Some(PresenceSection {
        present: stats.present,
        retard: stats.retard,
        absent: stats.absent,
        rate: Some(rate),
        recent: rows
            .into_iter()
            .map(|r| RecentPresence {
                id: r.id,
                title: r.titre,
                date: iso_db(r.event_date),
                status: r.status.unwrap_or_else(|| "NON_REPONDU".to_owned()),
            })
            .collect(),
    }))
}

/// Dernières absences déclarées (fin anticipée prise en compte, annulées avant leur début exclues).
async fn absences(db: &SqlitePool, guild: &str, user: &str) -> Result<Vec<AbsenceItem>> {
    let rows: Vec<(String, DbTime, DbTime, Option<DbTime>)> = sqlx::query_as(
        r#"SELECT "motif", "startsAt", MIN("endsAt", COALESCE("endedAt", "endsAt")), "endedAt" FROM "Absence"
           WHERE "guildId" = ? AND "userId" = ? AND MIN("endsAt", COALESCE("endedAt", "endsAt")) > "startsAt"
           ORDER BY "startsAt" DESC LIMIT ?"#,
    )
    .bind(guild)
    .bind(user)
    .bind(RECENT)
    .fetch_all(db)
    .await?;
    let now = DbTime::now();
    Ok(rows
        .into_iter()
        .map(|(motif, from, to, ended)| AbsenceItem {
            from: iso_db(from),
            to: iso_db(to),
            motif,
            current: ended.is_none() && from <= now && to > now,
        })
        .collect())
}

async fn exists(db: &SqlitePool, sql: &str, guild: &str) -> Result<bool> {
    Ok(sqlx::query_as::<_, (i64,)>(sql).bind(guild).fetch_one(db).await?.0 != 0)
}

/// `null` tant que le serveur n'a aucune cotisation.
async fn cotisation_section(ctx: &Ctx, guild_id: Id<GuildMarker>, user: &str) -> Result<Option<CotisationSection>> {
    let guild = guild_id.to_string();
    if !exists(&ctx.db, r#"SELECT EXISTS (SELECT 1 FROM "Cotisation" WHERE "guildId" = ?)"#, &guild).await? {
        return Ok(None);
    }
    let filter = Filter { user_id: Some(user.to_owned()), ..Default::default() };
    let totals = cotisations_service::totals(&ctx.db, &guild, &filter).await?;
    let rows = cotisations_service::list(&ctx.db, &guild, &filter, RECENT, 0).await?;
    let weeks = series::last_weeks(paris_date(Utc::now()), PROFILE_WEEKS);
    let lookup = Members::new(ctx, guild_id);
    Ok(Some(CotisationSection {
        totals: totals.into(),
        weekly: cotisations::weekly(&ctx.db, &guild, Some(user), None, &weeks).await?,
        recent: rows.iter().map(|row| cotisations::entry(&lookup, row)).collect(),
        objective: None,
    }))
}

/// `null` tant que le module coffre n'est pas utilisé sur le serveur.
async fn coffre_section(ctx: &Ctx, guild_id: Id<GuildMarker>, user: &str) -> Result<Option<CoffreSection>> {
    let guild = guild_id.to_string();
    let used = exists(
        &ctx.db,
        r#"SELECT EXISTS (SELECT 1 FROM "CoffreChannel" WHERE "guildId" = ?1)
              OR EXISTS (SELECT 1 FROM "CoffreStock" WHERE "guildId" = ?1)"#,
        &guild,
    )
    .await?;
    if !used {
        return Ok(None);
    }
    let characters: Vec<(String, Option<String>)> = sqlx::query_as(
        r#"SELECT p."gameId",
                  (SELECT m."playerName" FROM "CoffreMovement" m
                   WHERE m."guildId" = p."guildId" AND m."playerGameId" = p."gameId" ORDER BY m."at" DESC LIMIT 1)
           FROM "CoffrePlayer" p WHERE p."guildId" = ? AND p."userId" = ? ORDER BY p."gameId""#,
    )
    .bind(&guild)
    .bind(user)
    .fetch_all(&ctx.db)
    .await?;
    let game_ids: Vec<String> = characters.iter().map(|(id, _)| id.clone()).collect();
    let filter = coffres_service::Filter { player_game_ids: Some(game_ids.clone()), ..Default::default() };
    let totals = coffres_service::by_player(&ctx.db, &guild, &filter).await?;
    let (deposited, withdrawn) = totals.iter().fold((0, 0), |(d, w), row| (d + row.2, w + row.3));
    let recent = coffres::movement_rows(&ctx.db, &guild, &filter, RECENT, 0).await?;
    let names = coffres::property_names(&ctx.db, &guild).await?;
    let links: HashMap<String, String> = game_ids.iter().map(|id| (id.clone(), user.to_owned())).collect();
    let lookup = Members::new(ctx, guild_id);
    Ok(Some(CoffreSection {
        characters: characters
            .into_iter()
            .map(|(game_id, name)| Character { name: name.unwrap_or_else(|| game_id.clone()), game_id })
            .collect(),
        deposited,
        withdrawn,
        recent: recent.into_iter().map(|row| coffres::movement(&lookup, &names, &links, row)).collect(),
    }))
}

/// Semaines d'avis, de la plus récente à la plus ancienne ; `null` si jamais recrue.
async fn recrue(db: &SqlitePool, lookup: &Members<'_>, guild: &str, user: &str) -> Result<Option<RecrueSection>> {
    let entries: Vec<(String, DbTime, Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT e."id", s."weekStart", e."vetoById", e."vetoMotif" FROM "RecrueEntry" e
           JOIN "RecrueSession" s ON s."id" = e."sessionId"
           WHERE e."guildId" = ? AND e."userId" = ? ORDER BY s."weekStart" DESC"#,
    )
    .bind(guild)
    .bind(user)
    .fetch_all(db)
    .await?;
    if entries.is_empty() {
        return Ok(None);
    }
    let votes: Vec<(String, String, i64)> = sqlx::query_as(
        r#"SELECT v."entryId", v."choix", COUNT(*) FROM "RecrueVote" v
           JOIN "RecrueEntry" e ON e."id" = v."entryId"
           WHERE e."guildId" = ? AND e."userId" = ? GROUP BY v."entryId", v."choix""#,
    )
    .bind(guild)
    .bind(user)
    .fetch_all(db)
    .await?;
    let count =
        |entry: &str, choix: &str| votes.iter().find(|(e, c, _)| e == entry && c == choix).map_or(0, |(_, _, n)| *n);
    Ok(Some(RecrueSection {
        weeks: entries
            .iter()
            .map(|(id, week, veto_by, motif)| RecrueWeek {
                week_start: paris_date(week.0).format("%Y-%m-%d").to_string(),
                pour: count(id, "POUR"),
                contre: count(id, "CONTRE"),
                blanc: count(id, "BLANC"),
                veto: veto_by.as_ref().map(|by| Veto { by: lookup.get(by), motif: motif.clone().unwrap_or_default() }),
            })
            .collect(),
    }))
}
