//! Présences : liste et détail (réponses, sans réponse, changements d'avis),
//! assiduité par membre et par semaine (règle du rapport hebdomadaire),
//! plannings récurrents, création et clôture depuis le site. Les écritures
//! publient et mettent à jour le message Discord comme `/presence`.

use super::{
    ApiError, ApiQuery, ApiResult, JsonBody, MemberRef, Members, Page, Session, clamp_page, created, iso_db, json,
    no_content, page_number,
    series::{self, paris_date},
    stats::{AttendanceWeek, Tracked, attendance_weekly},
};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        permissions::Perm,
        presences::{
            blocklist,
            model::{Presence, PresenceResponse, ResponseStatus},
            planning::{self, DEFAULT_LEAD_HOURS, LEAD_HOURS, Schedule},
            report::{self, WindowData},
            service::{self, NewPresence},
        },
    },
    util::{
        ids::new_id,
        time::{DbTime, paris_day_start, parse_iso_day, resolve_event_date},
    },
    web::transactions::{number_cell, text_cell},
};
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::{IntoParams, ToSchema};

const PAGE_SIZE: i64 = 20;
const NOT_FOUND: &str = "Présence introuvable.";
const TITLE_MAX: usize = 100;
const DESCRIPTION_MAX: usize = 1000;
/// Semaines du graphique quand la période est « tout ».
const ALL_TIME_WEEKS: i64 = 12;
const DEFAULT_DAYS: i64 = 30;

// ---------------------------------------------------------------------------
// Formes JSON

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PresenceSummary)]
pub struct Summary {
    pub id: String,
    pub title: String,
    #[schema(required = true)]
    pub description: Option<String>,
    /// Heure de l'événement.
    pub date: String,
    /// `null` : ouverte.
    #[schema(required = true)]
    pub closed_at: Option<String>,
    pub created_by: MemberRef,
    pub present: usize,
    pub absent: usize,
    pub retard: usize,
    /// Membres actuels du rôle sans réponse (comme le message Discord).
    pub no_answer: usize,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PresenceRespondent)]
pub struct Respondent {
    pub member: MemberRef,
    #[schema(required = true)]
    pub retard_minutes: Option<i64>,
    /// A changé d'avis au moins une fois.
    pub changed: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PresenceChange)]
pub struct Change {
    pub member: MemberRef,
    #[schema(value_type = Option<super::openapi::PresenceStatus>, required = true)]
    pub from: Option<String>,
    #[schema(value_type = super::openapi::PresenceStatus)]
    pub to: String,
    pub at: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresenceDetail {
    pub presence: Summary,
    pub present: Vec<Respondent>,
    pub absent: Vec<Respondent>,
    pub retard: Vec<Respondent>,
    pub no_answer: Vec<MemberRef>,
    /// Changements d'avis, plus récents d'abord.
    pub changes: Vec<Change>,
    /// Effectif actuel du rôle des présences.
    pub roster: usize,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberAttendance {
    pub member: MemberRef,
    /// Présences clôturées où le membre était attendu.
    pub expected: i64,
    pub present: i64,
    pub retard: i64,
    pub absent: i64,
    pub no_answer: i64,
    /// Présents ÷ attendus, arrondi ; `null` sans présence attendue.
    #[schema(required = true)]
    pub rate: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresenceStats {
    /// Présences clôturées de la période.
    pub presences: usize,
    #[schema(required = true)]
    pub rate: Option<i64>,
    /// Seuil d'alerte du rapport hebdomadaire, en %.
    pub threshold: i64,
    /// Du plus assidu au moins assidu.
    pub members: Vec<MemberAttendance>,
    /// Semaines de la période (12 dernières pour « tout »), vides comprises.
    pub weekly: Vec<AttendanceWeek>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PresenceSchedule {
    pub id: String,
    /// 1 = lundi … 7 = dimanche.
    pub weekday: i64,
    pub weekday_label: &'static str,
    /// `HH:MM`, heure de Paris.
    pub time: String,
    pub title: String,
    #[schema(required = true)]
    pub description: Option<String>,
    /// Publication, en heures avant l'événement.
    pub lead_hours: i64,
    #[schema(required = true)]
    pub next_event: Option<String>,
    pub created_by: MemberRef,
}

// ---------------------------------------------------------------------------
// Calculs

fn count(responses: &[PresenceResponse], status: ResponseStatus) -> usize {
    responses.iter().filter(|r| r.status() == status).count()
}

fn summary(ctx: &Ctx, members: &Members, presence: &Presence, responses: &[PresenceResponse]) -> Summary {
    let responded: HashSet<&str> = responses.iter().map(|r| r.user_id.as_str()).collect();
    let roster = service::compute_roster(ctx, members.guild_id, &presence.role_id, &responded);
    Summary {
        id: presence.id.clone(),
        title: presence.titre.clone(),
        description: presence.description.clone(),
        date: iso_db(presence.event_date),
        closed_at: presence.closed_at.map(iso_db),
        created_by: members.get(&presence.created_by_id),
        present: count(responses, ResponseStatus::Present),
        absent: count(responses, ResponseStatus::Absent),
        retard: count(responses, ResponseStatus::Retard),
        no_answer: roster.non_respondent_ids.len(),
    }
}

/// Assiduité d'un membre sur une période.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub expected: i64,
    pub present: i64,
    pub retard: i64,
    pub absent: i64,
}

impl Tally {
    pub fn no_answer(&self) -> i64 {
        self.expected - self.present - self.retard - self.absent
    }

    pub fn rate(&self) -> Option<i64> {
        (self.expected > 0).then(|| (self.present as f64 * 100.0 / self.expected as f64).round() as i64)
    }
}

/// Assiduité par membre, règle du rapport hebdomadaire : sont attendus à une
/// présence les membres suivis déjà arrivés, plus ceux qui y ont répondu (hors
/// blocklist). Un retard n'est pas compté présent.
pub fn member_tallies(data: &WindowData, tracked: &[Tracked], blocklisted: &HashSet<String>) -> HashMap<String, Tally> {
    let mut by_presence: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for response in &data.responses {
        if !blocklisted.contains(&response.user_id) {
            by_presence.entry(&response.presence_id).or_default().insert(&response.user_id, &response.status);
        }
    }
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    for presence in &data.presences {
        let statuses = by_presence.get(presence.id.as_str());
        let mut expected: HashSet<&str> = tracked
            .iter()
            .filter(|m| m.joined_at.is_none_or(|joined| joined <= presence.event_date))
            .map(|m| m.user_id.as_str())
            .collect();
        if let Some(statuses) = statuses {
            expected.extend(statuses.keys().copied());
        }
        for user_id in expected {
            let tally = tallies.entry(user_id.to_owned()).or_default();
            tally.expected += 1;
            match statuses.and_then(|s| s.get(user_id)).and_then(|s| ResponseStatus::parse(s)) {
                Some(ResponseStatus::Present) => tally.present += 1,
                Some(ResponseStatus::Retard) => tally.retard += 1,
                Some(ResponseStatus::Absent) => tally.absent += 1,
                None => {}
            }
        }
    }
    tallies
}

/// Classement : meilleur taux d'abord, puis le plus de présences attendues.
pub fn ranked(tallies: HashMap<String, Tally>) -> Vec<(String, Tally)> {
    let mut rows: Vec<(String, Tally)> = tallies.into_iter().collect();
    rows.sort_by(|(a_id, a), (b_id, b)| {
        // b.present / b.expected comparé à a.present / a.expected, sans flottants.
        (b.present * a.expected).cmp(&(a.present * b.expected)).then(b.expected.cmp(&a.expected)).then(a_id.cmp(b_id))
    });
    rows
}

/// Période d'assiduité : les N derniers jours (`None` : tout).
fn parse_days(raw: Option<&str>) -> Option<i64> {
    match raw.map(str::trim) {
        Some("tout") => None,
        Some(value) => Some(value.parse::<i64>().ok().filter(|d| (1..=3650).contains(d)).unwrap_or(DEFAULT_DAYS)),
        None => Some(DEFAULT_DAYS),
    }
}

/// Bornes `[from, to[` de la période et semaines du graphique.
fn period(days: Option<i64>, now: DateTime<Utc>) -> (DbTime, DbTime, Vec<NaiveDate>) {
    let today = paris_date(now);
    let to = paris_day_start(today + Duration::days(1));
    match days {
        Some(days) => {
            let first = today - Duration::days(days - 1);
            (paris_day_start(first), to, series::weeks(first, today))
        }
        None => (DbTime(DateTime::UNIX_EPOCH), to, series::last_weeks(today, ALL_TIME_WEEKS)),
    }
}

fn tracked(ctx: &Ctx, guild_id: Id<GuildMarker>, role: Option<&str>, blocklisted: &HashSet<String>) -> Vec<Tracked> {
    role.map(|role| report::tracked_members(ctx, guild_id, role, blocklisted))
        .unwrap_or_default()
        .into_iter()
        .map(|m| Tracked { user_id: m.user_id, joined_at: m.joined_at })
        .collect()
}

async fn build_stats(ctx: &Ctx, guild_id: Id<GuildMarker>, days: Option<i64>) -> Result<PresenceStats> {
    let guild = guild_id.to_string();
    let (from, to, weeks) = period(days, Utc::now());
    let config = service::get_config(&ctx.db, &guild).await?;
    let blocklisted: HashSet<String> = blocklist::user_ids(&ctx.db, &guild).await?.into_iter().collect();
    let tracked = tracked(ctx, guild_id, config.as_ref().and_then(|c| c.default_role_id.as_deref()), &blocklisted);
    let data = report::gather(&ctx.db, &guild, from, to).await?;
    // Le graphique couvre des semaines entières, parfois au-delà de la période.
    let weekly = match (weeks.first(), weeks.last()) {
        (Some(first), Some(last)) => {
            let span =
                report::gather(&ctx.db, &guild, paris_day_start(*first), paris_day_start(*last + Duration::weeks(1)))
                    .await?;
            attendance_weekly(&weeks, &span, &tracked, &blocklisted)
        }
        _ => Vec::new(),
    };
    let members = Members::new(ctx, guild_id);
    let rows = ranked(member_tallies(&data, &tracked, &blocklisted));
    let (present, expected) = rows.iter().fold((0, 0), |(p, e), (_, t)| (p + t.present, e + t.expected));
    Ok(PresenceStats {
        presences: data.presences.len(),
        rate: Tally { expected, present, ..Tally::default() }.rate(),
        threshold: config.map_or(crate::modules::presences::model::DEFAULT_ALERT_THRESHOLD, |c| c.alert_threshold()),
        members: rows
            .into_iter()
            .map(|(user_id, t)| MemberAttendance {
                member: members.get(&user_id),
                expected: t.expected,
                present: t.present,
                retard: t.retard,
                absent: t.absent,
                no_answer: t.no_answer(),
                rate: t.rate(),
            })
            .collect(),
        weekly,
    })
}

fn build_csv(rows: &[MemberAttendance]) -> String {
    // BOM : sans lui, Excel lit le fichier en Windows-1252 et casse les accents.
    let mut csv = String::from("\u{feff}Membre;Attendu;Présent;Retard;Absent;Sans réponse;Taux (%)\r\n");
    for row in rows {
        let cells = [
            text_cell(&row.member.name),
            number_cell(row.expected as f64),
            number_cell(row.present as f64),
            number_cell(row.retard as f64),
            number_cell(row.absent as f64),
            number_cell(row.no_answer as f64),
            row.rate.map(|r| number_cell(r as f64)).unwrap_or_default(),
        ];
        csv.push_str(&cells.join(";"));
        csv.push_str("\r\n");
    }
    csv
}

// ---------------------------------------------------------------------------
// Lecture

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PresenceQuery {
    /// `a_venir` (ouvertes, pas encore passées), `ouvertes` ou `closes` ; absent : toutes.
    etat: Option<String>,
    /// Premier jour (`YYYY-MM-DD`, heure de Paris).
    du: Option<String>,
    /// Dernier jour inclus.
    au: Option<String>,
    /// Page (1 = première).
    page: Option<String>,
}

/// Condition SQL et ordre d'une liste de présences.
fn list_filter(query: &PresenceQuery) -> (String, Vec<String>, &'static str) {
    let mut clauses = vec![r#""guildId" = ?"#.to_owned()];
    let mut binds = Vec::new();
    let order = match query.etat.as_deref() {
        Some("a_venir") => {
            clauses.push(r#""closedAt" IS NULL AND "eventDate" > ?"#.to_owned());
            binds.push(DbTime::now().to_db_string());
            "ASC"
        }
        Some("ouvertes") => {
            clauses.push(r#""closedAt" IS NULL"#.to_owned());
            "ASC"
        }
        Some("closes") => {
            clauses.push(r#""closedAt" IS NOT NULL"#.to_owned());
            "DESC"
        }
        _ => "DESC",
    };
    if let Some(day) = query.du.as_deref().and_then(parse_iso_day) {
        clauses.push(r#""eventDate" >= ?"#.to_owned());
        binds.push(paris_day_start(day).to_db_string());
    }
    if let Some(day) = query.au.as_deref().and_then(parse_iso_day) {
        clauses.push(r#""eventDate" < ?"#.to_owned());
        binds.push(paris_day_start(day + Duration::days(1)).to_db_string());
    }
    (clauses.join(" AND "), binds, order)
}

async fn responses_of(ctx: &Ctx, presences: &[Presence]) -> Result<HashMap<String, Vec<PresenceResponse>>> {
    let mut by_id = HashMap::new();
    for presence in presences {
        by_id.insert(presence.id.clone(), service::responses(&ctx.db, &presence.id).await?);
    }
    Ok(by_id)
}

/// Présences du serveur, paginées (`presence.stats`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/presences", tag = "presences", operation_id = "getPresences",
    params(("guild" = String, Path, description = "Identifiant du serveur"), PresenceQuery),
    responses(
        (status = 200, description = "Présences", body = super::openapi::PresenceSummaryPage),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn list(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<PresenceQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceStats)?;
    let (filter, binds, order) = list_filter(&query);
    let count_sql = format!(r#"SELECT COUNT(*) FROM "Presence" WHERE {filter}"#);
    let mut count = sqlx::query_as::<_, (i64,)>(&count_sql).bind(guild_id.to_string());
    for bind in &binds {
        count = count.bind(bind);
    }
    let (total,) = count.fetch_one(&ctx.db).await?;
    let (page, offset) = clamp_page(page_number(query.page.as_deref()), total, PAGE_SIZE);
    let sql = format!(
        r#"SELECT "id", "guildId", "channelId", "messageId", "titre", "description", "eventDate", "roleId", "createdById", "createdAt", "closedAt"
           FROM "Presence" WHERE {filter} ORDER BY "eventDate" {order}, "id" LIMIT ? OFFSET ?"#
    );
    let mut rows = sqlx::query_as::<_, Presence>(&sql).bind(guild_id.to_string());
    for bind in &binds {
        rows = rows.bind(bind);
    }
    let presences = rows.bind(PAGE_SIZE).bind(offset).fetch_all(&ctx.db).await?;
    let responses = responses_of(&ctx, &presences).await?;
    let members = Members::new(&ctx, guild_id);
    let items =
        presences.iter().map(|p| summary(&ctx, &members, p, responses.get(&p.id).map_or(&[], Vec::as_slice))).collect();
    Ok(json(&Page { items, total, page, page_size: PAGE_SIZE }))
}

async fn find(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> ApiResult<Presence> {
    service::get_guild_presence(&ctx.db, &guild_id.to_string(), id.trim())
        .await?
        .ok_or_else(|| ApiError::not_found(NOT_FOUND))
}

async fn detail_of(ctx: &Ctx, guild_id: Id<GuildMarker>, presence: &Presence) -> Result<PresenceDetail> {
    let members = Members::new(ctx, guild_id);
    let responses = service::responses(&ctx.db, &presence.id).await?;
    let responded: HashSet<&str> = responses.iter().map(|r| r.user_id.as_str()).collect();
    let roster = service::compute_roster(ctx, guild_id, &presence.role_id, &responded);
    let by_status = |status: ResponseStatus| {
        responses
            .iter()
            .filter(|r| r.status() == status)
            .map(|r| Respondent {
                member: members.get(&r.user_id),
                retard_minutes: r.retard_minutes,
                changed: r.changed,
            })
            .collect::<Vec<_>>()
    };
    let changes = service::presence_changes(&ctx.db, &presence.id).await?;
    let mut changes: Vec<Change> = changes
        .into_iter()
        .map(|c| Change {
            member: members.get(&c.user_id),
            from: c.from_status,
            to: c.to_status,
            at: iso_db(c.changed_at),
        })
        .collect();
    changes.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(PresenceDetail {
        presence: summary(ctx, &members, presence, &responses),
        present: by_status(ResponseStatus::Present),
        absent: by_status(ResponseStatus::Absent),
        retard: by_status(ResponseStatus::Retard),
        no_answer: roster.non_respondent_ids.iter().map(|id| members.get(id)).collect(),
        changes,
        roster: roster.effectif_total,
    })
}

/// Détail d'une présence (`presence.stats`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/presences/{id}", tag = "presences", operation_id = "getPresence",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la présence")),
    responses(
        (status = 200, description = "Présence", body = PresenceDetail),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn detail(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceStats)?;
    let presence = find(&ctx, guild_id, &id).await?;
    Ok(json(&detail_of(&ctx, guild_id, &presence).await?))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StatsQuery {
    /// Les N derniers jours (30 par défaut), `tout` : depuis le début.
    jours: Option<String>,
}

/// Assiduité par membre et par semaine (`presence.stats`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/presences/stats", tag = "presences", operation_id = "getPresenceStats",
    params(("guild" = String, Path, description = "Identifiant du serveur"), StatsQuery),
    responses(
        (status = 200, description = "Assiduité", body = PresenceStats),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn stats(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<StatsQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceStats)?;
    Ok(json(&build_stats(&ctx, guild_id, parse_days(query.jours.as_deref())).await?))
}

/// Export CSV de l'assiduité par membre, même période que la page.
#[utoipa::path(
    get, path = "/api/guilds/{guild}/presences/export.csv", tag = "presences", operation_id = "exportPresenceStats",
    params(("guild" = String, Path, description = "Identifiant du serveur"), StatsQuery),
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
    ApiQuery(query): ApiQuery<StatsQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceStats)?;
    let stats = build_stats(&ctx, guild_id, parse_days(query.jours.as_deref())).await?;
    let filename = format!("assiduite-{guild_id}-{}.csv", Utc::now().format("%Y-%m-%d"));
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        build_csv(&stats.members),
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// Création et clôture

/// Nouvelle présence : jour (aujourd'hui, ou demain si l'heure est passée) et heure de Paris.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewPresenceInput {
    /// `YYYY-MM-DD` ; absent : aujourd'hui, ou demain si l'heure est passée.
    #[serde(default)]
    pub date: Option<String>,
    /// `HH:MM`.
    pub time: String,
    /// Absent : « Présence ». 100 caractères au plus.
    #[serde(default)]
    pub title: Option<String>,
    /// 1000 caractères au plus.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct CheckedPresence {
    pub event_date: DbTime,
    pub title: String,
    pub description: Option<String>,
}

fn optional(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

/// Saisie validée comme `/presence create` (date dans le passé refusée).
pub fn check_presence(input: NewPresenceInput, now: DateTime<Utc>) -> Result<CheckedPresence, String> {
    let date = match optional(input.date) {
        Some(raw) => Some(parse_iso_day(&raw).ok_or("Date invalide.")?.format("%d/%m/%Y").to_string()),
        None => None,
    };
    let resolved =
        resolve_event_date(date.as_deref(), &input.time).ok_or("Date ou heure invalide (heure au format HH:MM).")?;
    if resolved.date.0 <= now {
        return Err("Cette date est déjà passée.".to_owned());
    }
    let title = optional(input.title).unwrap_or_else(|| "Présence".to_owned());
    if title.chars().count() > TITLE_MAX {
        return Err(format!("Le titre dépasse {TITLE_MAX} caractères."));
    }
    let description = optional(input.description);
    if description.as_ref().is_some_and(|d| d.chars().count() > DESCRIPTION_MAX) {
        return Err(format!("La description dépasse {DESCRIPTION_MAX} caractères."));
    }
    Ok(CheckedPresence { event_date: resolved.date, title, description })
}

/// Crée et publie une présence dans le salon des présences, avec le ping habituel (`presence.creer`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/presences", tag = "presences", operation_id = "createPresence",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = NewPresenceInput,
    responses(
        (status = 201, description = "Présence publiée", body = Summary),
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
    JsonBody(input): JsonBody<NewPresenceInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceCreer)?;
    let checked = check_presence(input, Utc::now()).map_err(ApiError::invalid)?;
    let config = service::get_config(&ctx.db, &guild_id.to_string()).await?;
    let Some(config) = config.filter(|c| c.default_role_id.is_some()) else {
        return Err(ApiError::invalid(
            "Aucun rôle configuré. Un admin doit d'abord lancer « /config presence role:@TonRôle » sur Discord.",
        ));
    };
    // Depuis le site, pas de salon « courant » : celui des présences est obligatoire.
    let Some(channel_id) = config.presence_channel_id.clone() else {
        return Err(ApiError::invalid(
            "Aucun salon des présences configuré : « /config presence channel:#salon » sur Discord.",
        ));
    };
    let published = service::publish_presence(
        &ctx,
        guild_id,
        NewPresence {
            channel_id,
            titre: checked.title,
            description: checked.description,
            event_date: checked.event_date,
            role_id: config.default_role_id.clone().unwrap_or_default(),
            absence_role_id: config.absence_role_id.clone(),
            created_by_id: session.viewer.id.to_string(),
        },
    )
    .await?;
    let Some(presence) = published else {
        return Err(ApiError::invalid(
            "La présence n'a pas pu être publiée (Discord a refusé la requête ou le salon est inaccessible).",
        ));
    };
    let members = Members::new(&ctx, guild_id);
    let responses = service::responses(&ctx.db, &presence.id).await?;
    Ok(created(&summary(&ctx, &members, &presence, &responses)))
}

/// Clôture une présence ouverte (`presence.creer`, comme `/presence close`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/presences/{id}/close", tag = "presences", operation_id = "closePresence",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de la présence")),
    responses(
        (status = 200, description = "Présence clôturée", body = PresenceDetail),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Déjà clôturée", body = super::ErrorBody),
    )
)]
pub async fn close(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceCreer)?;
    let presence = find(&ctx, guild_id, &id).await?;
    if presence.closed_at.is_some() {
        return Err(ApiError::invalid("Cette présence est déjà clôturée."));
    }
    service::close_presence(&ctx.db, &presence.id).await?;
    if let Err(error) = service::sync_message(&ctx, &presence.id).await {
        tracing::warn!(presence = %presence.id, ?error, "message de la présence non mis à jour");
    }
    let presence = find(&ctx, guild_id, &presence.id).await?;
    Ok(json(&detail_of(&ctx, guild_id, &presence).await?))
}

// ---------------------------------------------------------------------------
// Plannings

fn schedule_json(members: &Members, schedule: &Schedule) -> PresenceSchedule {
    PresenceSchedule {
        id: schedule.id.clone(),
        weekday: schedule.weekday,
        weekday_label: schedule.weekday_label(),
        time: schedule.time.clone(),
        title: schedule.titre.clone(),
        description: schedule.description.clone(),
        lead_hours: schedule.lead_hours,
        next_event: planning::next_event(schedule.weekday, &schedule.time, Utc::now()).map(iso_db),
        created_by: members.get(&schedule.created_by_id),
    }
}

/// Présences récurrentes (`presence.gerer`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/presences/plannings", tag = "presences", operation_id = "getPresenceSchedules",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Plannings", body = Vec<PresenceSchedule>),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn schedules(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceGerer)?;
    let members = Members::new(&ctx, guild_id);
    let list = planning::list(&ctx.db, &guild_id.to_string()).await?;
    Ok(json(&list.iter().map(|s| schedule_json(&members, s)).collect::<Vec<_>>()))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewScheduleInput {
    /// 1 = lundi … 7 = dimanche.
    pub weekday: i64,
    /// `HH:MM`, heure de Paris.
    pub time: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// 1 à 144 heures avant l'événement (24 par défaut).
    #[serde(default)]
    pub lead_hours: Option<i64>,
}

/// Planning validé comme `/gestion presence planifier`.
pub fn check_schedule(input: NewScheduleInput, guild_id: &str, created_by_id: &str) -> Result<Schedule, String> {
    if !(1..=7).contains(&input.weekday) {
        return Err("Jour invalide.".to_owned());
    }
    let time = planning::normalize_time(&input.time).ok_or("Heure invalide. Format attendu : HH:MM.")?;
    let lead_hours = input.lead_hours.unwrap_or(DEFAULT_LEAD_HOURS);
    if !LEAD_HOURS.contains(&lead_hours) {
        return Err(format!(
            "La publication se fait entre {} et {} heures avant l'événement.",
            LEAD_HOURS.start(),
            LEAD_HOURS.end()
        ));
    }
    let titre = optional(input.title).unwrap_or_else(|| "Présence".to_owned());
    if titre.chars().count() > TITLE_MAX {
        return Err(format!("Le titre dépasse {TITLE_MAX} caractères."));
    }
    let description = optional(input.description);
    if description.as_ref().is_some_and(|d| d.chars().count() > DESCRIPTION_MAX) {
        return Err(format!("La description dépasse {DESCRIPTION_MAX} caractères."));
    }
    Ok(Schedule {
        id: new_id(),
        guild_id: guild_id.to_owned(),
        weekday: input.weekday,
        time,
        titre,
        description,
        lead_hours,
        last_event_date: None,
        created_by_id: created_by_id.to_owned(),
    })
}

async fn journal(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, action: Action, details: String) {
    let actor = session.actor_name(ctx, guild_id);
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, Entry::new(action, None, details)).await;
}

/// Ajoute une présence récurrente (`presence.gerer`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/presences/plannings", tag = "presences", operation_id = "createPresenceSchedule",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = NewScheduleInput,
    responses(
        (status = 201, description = "Planning ajouté", body = PresenceSchedule),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn create_schedule(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(input): JsonBody<NewScheduleInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceGerer)?;
    let schedule =
        check_schedule(input, &guild_id.to_string(), &session.viewer.id.to_string()).map_err(ApiError::invalid)?;
    planning::insert(&ctx.db, &schedule).await?;
    journal(&ctx, &session, guild_id, Action::PresenceSchedule, schedule.summary()).await;
    Ok(created(&schedule_json(&Members::new(&ctx, guild_id), &schedule)))
}

/// Supprime une présence récurrente ; les présences déjà publiées restent (`presence.gerer`).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/presences/plannings/{id}", tag = "presences", operation_id = "deletePresenceSchedule",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du planning")),
    responses(
        (status = 204, description = "Supprimé"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn delete_schedule(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::PresenceGerer)?;
    let gid = guild_id.to_string();
    let schedule = planning::list(&ctx.db, &gid).await?.into_iter().find(|s| s.id == id.trim());
    let Some(schedule) = schedule else {
        return Err(ApiError::not_found("Présence planifiée introuvable."));
    };
    planning::delete(&ctx.db, &gid, &schedule.id).await?;
    journal(&ctx, &session, guild_id, Action::PresenceUnschedule, schedule.summary()).await;
    Ok(no_content())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::presences::report::{ClosedPresence, ResponseRow};
    use chrono::TimeZone;

    fn at(d: u32, h: u32) -> DbTime {
        DbTime(Utc.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap())
    }

    #[test]
    fn tallies_follow_the_report_rule() {
        let presence = |id: &str, date| ClosedPresence { id: id.into(), titre: id.into(), event_date: date };
        let response =
            |p: &str, u: &str, s: &str| ResponseRow { presence_id: p.into(), user_id: u.into(), status: s.into() };
        let data = WindowData {
            presences: vec![presence("p1", at(6, 19)), presence("p2", at(8, 19))],
            responses: vec![
                response("p1", "a", "PRESENT"),
                response("p1", "b", "RETARD"),
                response("p2", "a", "PRESENT"),
                response("p2", "b", "ABSENT"),
                response("p2", "x", "PRESENT"),
                response("p2", "banni", "PRESENT"),
            ],
            absences: Vec::new(),
        };
        let tracked = vec![
            Tracked { user_id: "a".into(), joined_at: None },
            Tracked { user_id: "b".into(), joined_at: None },
            Tracked { user_id: "c".into(), joined_at: Some(at(7, 0)) },
        ];
        let blocklisted: HashSet<String> = ["banni".to_owned()].into();
        let tallies = member_tallies(&data, &tracked, &blocklisted);
        assert_eq!(tallies["a"], Tally { expected: 2, present: 2, retard: 0, absent: 0 });
        assert_eq!(tallies["b"], Tally { expected: 2, present: 0, retard: 1, absent: 1 });
        // Arrivé après la première présence : attendu une fois, sans réponse.
        assert_eq!((tallies["c"].expected, tallies["c"].no_answer()), (1, 1));
        // A répondu sans porter le rôle : attendu là où il a répondu.
        assert_eq!(tallies["x"], Tally { expected: 1, present: 1, retard: 0, absent: 0 });
        assert!(!tallies.contains_key("banni"));

        let order: Vec<String> = ranked(tallies).into_iter().map(|(id, _)| id).collect();
        assert_eq!(order, ["a", "x", "b", "c"]);
        assert_eq!(Tally { expected: 3, present: 2, ..Tally::default() }.rate(), Some(67));
        assert_eq!(Tally::default().rate(), None);
    }

    #[test]
    fn periods_cover_whole_paris_days() {
        // Mardi 20 octobre 2026, 10:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 10, 20, 10, 0, 0).unwrap();
        let (from, to, weeks) = period(Some(7), now);
        assert_eq!(from.0, Utc.with_ymd_and_hms(2026, 10, 13, 22, 0, 0).unwrap());
        assert_eq!(to.0, Utc.with_ymd_and_hms(2026, 10, 20, 22, 0, 0).unwrap());
        assert_eq!(weeks.len(), 2);
        let (_, _, all) = period(None, now);
        assert_eq!(all.len(), 12);
        assert_eq!((parse_days(None), parse_days(Some("tout")), parse_days(Some("x"))), (Some(30), None, Some(30)));
        assert_eq!(parse_days(Some("90")), Some(90));
    }

    #[test]
    fn checks_new_presences_and_schedules() {
        let now = Utc.with_ymd_and_hms(2026, 10, 20, 10, 0, 0).unwrap();
        let input = |date: Option<&str>, time: &str, title: Option<&str>| NewPresenceInput {
            date: date.map(Into::into),
            time: time.into(),
            title: title.map(Into::into),
            description: Some("  ".into()),
        };
        let ok = check_presence(input(Some("2099-10-21"), "21:00", Some("  Braquage ")), now).unwrap();
        assert_eq!((ok.title.as_str(), ok.description), ("Braquage", None));
        assert!(check_presence(input(Some("2026-10-19"), "21:00", None), now).unwrap_err().contains("passée"));
        assert!(check_presence(input(Some("21/10/2099"), "21:00", None), now).unwrap_err().contains("Date"));
        assert!(check_presence(input(None, "25:00", None), now).unwrap_err().contains("heure"));
        assert!(check_presence(input(Some("2099-10-21"), "21:00", Some(&"x".repeat(101))), now).is_err());

        let schedule = |weekday, time: &str, lead| NewScheduleInput {
            weekday,
            time: time.into(),
            title: None,
            description: None,
            lead_hours: lead,
        };
        let ok = check_schedule(schedule(3, "21h00", None), "1", "2").unwrap();
        assert_eq!((ok.time.as_str(), ok.lead_hours, ok.titre.as_str()), ("21:00", 24, "Présence"));
        assert!(check_schedule(schedule(8, "21:00", None), "1", "2").unwrap_err().contains("Jour"));
        assert!(check_schedule(schedule(3, "21:00", Some(200)), "1", "2").unwrap_err().contains("144"));
        assert!(check_schedule(schedule(3, "midi", None), "1", "2").unwrap_err().contains("Heure"));
    }

    #[test]
    fn csv_lists_members() {
        let row = MemberAttendance {
            member: MemberRef { id: "1".into(), name: "=Elodie".into(), avatar_url: None },
            expected: 4,
            present: 3,
            retard: 1,
            absent: 0,
            no_answer: 0,
            rate: Some(75),
        };
        let csv = build_csv(&[row]);
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.ends_with("'=Elodie;4;3;1;0;0;75\r\n"), "{csv}");
    }

    #[test]
    fn list_filters_build_sql() {
        let query = PresenceQuery { etat: Some("closes".into()), du: Some("2026-10-01".into()), au: None, page: None };
        let (sql, binds, order) = list_filter(&query);
        assert_eq!(sql, r#""guildId" = ? AND "closedAt" IS NOT NULL AND "eventDate" >= ?"#);
        assert_eq!((binds.len(), order), (1, "DESC"));
        let (_, _, order) = list_filter(&PresenceQuery { etat: Some("a_venir".into()), ..Default::default() });
        assert_eq!(order, "ASC");
    }
}
