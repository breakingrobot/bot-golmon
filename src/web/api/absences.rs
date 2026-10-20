//! Absences : en cours, à venir et récentes, déclaration et fin depuis le site,
//! avec les mêmes effets que `/absence` (annonce, rôle d'absence, présences
//! ouvertes passées en absent, journal pour un autre membre).

use super::{ApiError, ApiResult, JsonBody, MemberRef, Members, Session, created, iso_db, json};
use crate::{
    context::{Context, Ctx},
    discord::members::avatar_url,
    modules::{
        audit::{self, Action, Entry},
        permissions::Perm,
        presences::absences::{self, NewAbsence, RoleUpdate},
    },
    util::time::{DbTime, parse_event_date, parse_iso_day},
    web::Access,
};
use axum::extract::{Path, State};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use utoipa::ToSchema;

/// Absences terminées encore listées, en jours.
const RECENT_DAYS: i64 = 30;
const MOTIF_MAX: usize = 200;
const NOT_FOUND: &str = "Absence introuvable, ou déjà terminée.";

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AbsenceEntry {
    pub id: String,
    pub member: MemberRef,
    pub motif: String,
    /// Début (00:00, heure de Paris).
    pub from: String,
    /// Fin prévue (23:59 du dernier jour).
    pub to: String,
    /// Fin anticipée ou annulation ; `null` : en cours ou à venir.
    #[schema(required = true)]
    pub ended_at: Option<String>,
    pub created_by: MemberRef,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AbsencesResponse {
    /// Commencées, pas terminées (fin la plus proche d'abord).
    pub current: Vec<AbsenceEntry>,
    /// Pas encore commencées (début le plus proche d'abord).
    pub upcoming: Vec<AbsenceEntry>,
    /// Terminées ou annulées depuis 30 jours (plus récentes d'abord).
    pub recent: Vec<AbsenceEntry>,
}

#[derive(Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
struct Row {
    id: String,
    user_id: String,
    motif: String,
    starts_at: DbTime,
    ends_at: DbTime,
    ended_at: Option<DbTime>,
    created_by_id: String,
}

const COLUMNS: &str = r#""id", "userId", "motif", "startsAt", "endsAt", "endedAt", "createdById""#;

fn entry(members: &Members, row: &Row) -> AbsenceEntry {
    AbsenceEntry {
        id: row.id.clone(),
        member: members.get(&row.user_id),
        motif: row.motif.clone(),
        from: iso_db(row.starts_at),
        to: iso_db(row.ends_at),
        ended_at: row.ended_at.map(iso_db),
        created_by: members.get(&row.created_by_id),
    }
}

/// Membre du groupe ou « Gérer le serveur » : comme `/absence list`, ouvert au groupe.
pub fn check_group(access: &Access, guild_id: Id<GuildMarker>) -> ApiResult<()> {
    super::check_guild(access, guild_id, None)?;
    match access.guild(guild_id) {
        Some(guild) if guild.in_group() || guild.is_admin() => Ok(()),
        _ => Err(ApiError::forbidden("Réservé aux membres du groupe.")),
    }
}

/// Absences en cours, à venir et récentes (membres du groupe).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/absences", tag = "absences", operation_id = "getAbsences",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Absences", body = AbsencesResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Pas membre du groupe", body = super::ErrorBody),
    )
)]
pub async fn list(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = super::parse_guild(&guild)?;
    check_group(&session.access, guild_id)?;
    let now = DbTime::now();
    let recent_since = DbTime(now.0 - Duration::days(RECENT_DAYS));
    let rows: Vec<Row> = sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Absence" WHERE "guildId" = ?
           AND (("endedAt" IS NULL AND "endsAt" > ?) OR MIN("endsAt", COALESCE("endedAt", "endsAt")) >= ?)
           ORDER BY "startsAt" ASC"#
    ))
    .bind(guild_id.to_string())
    .bind(now)
    .bind(recent_since)
    .fetch_all(&ctx.db)
    .await?;
    let members = Members::new(&ctx, guild_id);
    let mut response = AbsencesResponse { current: Vec::new(), upcoming: Vec::new(), recent: Vec::new() };
    let mut recent: Vec<(DbTime, AbsenceEntry)> = Vec::new();
    for row in &rows {
        let finished = row.ended_at.is_some() || row.ends_at <= now;
        if finished {
            recent.push((row.ended_at.unwrap_or(row.ends_at).min(row.ends_at), entry(&members, row)));
        } else if row.starts_at <= now {
            response.current.push(entry(&members, row));
        } else {
            response.upcoming.push(entry(&members, row));
        }
    }
    response.current.sort_by(|a, b| a.to.cmp(&b.to));
    recent.sort_by_key(|a| std::cmp::Reverse(a.0));
    response.recent = recent.into_iter().map(|(_, e)| e).collect();
    Ok(json(&response))
}

/// Déclaration d'absence : dates en jours de Paris, journées entières.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewAbsenceInput {
    /// Absent : soi-même ; un autre membre demande `absence.autrui`.
    #[serde(default)]
    pub member_id: Option<String>,
    /// 200 caractères au plus.
    pub motif: String,
    /// Premier jour (`YYYY-MM-DD`) ; absent : aujourd'hui.
    #[serde(default)]
    pub from: Option<String>,
    /// Dernier jour inclus (`YYYY-MM-DD`).
    pub to: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredAbsence {
    pub absence: AbsenceEntry,
    /// Présences ouvertes de la période passées en absent.
    pub affected: usize,
    /// Rôle d'absence non posé : raison, sinon `null`.
    #[schema(required = true)]
    pub role_warning: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct CheckedAbsence {
    pub motif: String,
    pub starts_at: DbTime,
    pub ends_at: DbTime,
}

/// Une date couvre la journée entière : début à 00:00, fin à 23:59 (heure de Paris).
fn day_boundary(day: NaiveDate, end: bool) -> Option<DbTime> {
    parse_event_date(&day.format("%d/%m/%Y").to_string(), if end { "23:59" } else { "00:00" })
}

/// Saisie validée comme `/absence declare`.
pub fn check_absence(input: &NewAbsenceInput, today: NaiveDate, now: DbTime) -> Result<CheckedAbsence, String> {
    let motif = input.motif.trim().to_owned();
    if motif.is_empty() {
        return Err("Indique un motif.".to_owned());
    }
    if motif.chars().count() > MOTIF_MAX {
        return Err(format!("Le motif dépasse {MOTIF_MAX} caractères."));
    }
    let from = match input.from.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(raw) => parse_iso_day(raw),
        None => Some(today),
    };
    let to = parse_iso_day(&input.to);
    let (Some(starts_at), Some(ends_at)) =
        (from.and_then(|d| day_boundary(d, false)), to.and_then(|d| day_boundary(d, true)))
    else {
        return Err("Date invalide.".to_owned());
    };
    if ends_at <= starts_at {
        return Err("La date de fin doit être postérieure à la date de début.".to_owned());
    }
    if ends_at <= now {
        return Err("Cette absence est déjà terminée.".to_owned());
    }
    Ok(CheckedAbsence { motif, starts_at, ends_at })
}

fn role_warning(role: RoleUpdate) -> Option<String> {
    (!role.ok).then(|| role.reason.unwrap_or_else(|| "raison inconnue".to_owned()))
}

async fn find(ctx: &Ctx, guild_id: Id<GuildMarker>, id: &str) -> ApiResult<Row> {
    let row: Option<Row> = sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Absence" WHERE "id" = ? AND "guildId" = ? AND "endedAt" IS NULL AND "endsAt" > ?"#
    ))
    .bind(id.trim())
    .bind(guild_id.to_string())
    .bind(DbTime::now())
    .fetch_optional(&ctx.db)
    .await?;
    row.ok_or_else(|| ApiError::not_found(NOT_FOUND))
}

/// Pour un autre membre : `absence.autrui`. Pour soi : être du groupe (vérifié ensuite).
fn check_target(session: &Session, guild_id: Id<GuildMarker>, target: Id<UserMarker>) -> ApiResult<()> {
    if target != session.viewer.id && !session.can(guild_id, Perm::AbsenceAutrui) {
        return Err(session.denied(guild_id, Perm::AbsenceAutrui));
    }
    Ok(())
}

/// Déclare une absence (soi : membre du groupe ; un autre membre : `absence.autrui`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/absences", tag = "absences", operation_id = "createAbsence",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = NewAbsenceInput,
    responses(
        (status = 201, description = "Absence enregistrée", body = DeclaredAbsence),
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
    JsonBody(input): JsonBody<NewAbsenceInput>,
) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let target = match input.member_id.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        Some(raw) => super::parse_user(raw)?,
        None => session.viewer.id,
    };
    check_target(&session, guild_id, target)?;
    super::require_member(&ctx, guild_id, target).await?;
    let now = DbTime::now();
    let checked = check_absence(&input, super::series::paris_date(now.0), now).map_err(ApiError::invalid)?;
    let declared = absences::declare(
        &ctx,
        guild_id,
        NewAbsence {
            user_id: target.to_string(),
            motif: checked.motif.clone(),
            starts_at: checked.starts_at,
            ends_at: checked.ends_at,
            created_by_id: session.viewer.id.to_string(),
        },
    )
    .await?;
    let for_someone_else = target != session.viewer.id;
    let actor = session.actor_name(&ctx, guild_id);
    let avatar = ctx.cache.user(target).map(|u| avatar_url(&u));
    let embed = absences::announcement(
        &target.to_string(),
        checked.starts_at,
        checked.ends_at,
        &checked.motif,
        avatar,
        for_someone_else.then_some(actor.as_str()),
    );
    absences::announce(&ctx, guild_id, &declared.id, &embed).await?;
    if for_someone_else {
        let details =
            format!("Du <t:{}:d> au <t:{}:d> — {}", checked.starts_at.unix(), checked.ends_at.unix(), checked.motif);
        let entry = Entry::new(Action::AbsenceDeclaredFor, Some(target.to_string()), details);
        audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    }
    let row = Row {
        id: declared.id,
        user_id: target.to_string(),
        motif: checked.motif,
        starts_at: checked.starts_at,
        ends_at: checked.ends_at,
        ended_at: None,
        created_by_id: session.viewer.id.to_string(),
    };
    Ok(created(&DeclaredAbsence {
        absence: entry(&Members::new(&ctx, guild_id), &row),
        affected: declared.affected,
        role_warning: role_warning(declared.role),
    }))
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndedAbsence {
    /// `true` : l'absence n'avait pas commencé, elle est annulée.
    pub cancelled: bool,
    /// Rôle d'absence non retiré : raison (l'absence reste ouverte, le bot réessaiera).
    #[schema(required = true)]
    pub role_warning: Option<String>,
}

/// Termine une absence en cours, ou annule une absence à venir (soi, ou `absence.autrui`).
#[utoipa::path(
    post, path = "/api/guilds/{guild}/absences/{id}/end", tag = "absences", operation_id = "endAbsence",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant de l'absence")),
    responses(
        (status = 200, description = "Absence terminée ou annulée", body = EndedAbsence),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn end(State(ctx): State<Context>, session: Session, Path((guild, id)): Path<(String, String)>) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let row = find(&ctx, guild_id, &id).await?;
    let target = super::parse_user(&row.user_id)?;
    check_target(&session, guild_id, target)?;
    let cancelled = row.starts_at.0 > Utc::now();
    let role = absences::end(&ctx, guild_id, &row.id, &row.user_id).await?;
    if target != session.viewer.id {
        let verb = if cancelled { "annulée" } else { "clôturée" };
        let actor = session.actor_name(&ctx, guild_id);
        let entry = Entry::new(Action::AbsenceEndedFor, Some(row.user_id.clone()), format!("Absence {verb}"));
        audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    }
    Ok(json(&EndedAbsence { cancelled, role_warning: role_warning(role) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn input(from: Option<&str>, to: &str, motif: &str) -> NewAbsenceInput {
        NewAbsenceInput { member_id: None, motif: motif.into(), from: from.map(Into::into), to: to.into() }
    }

    #[test]
    fn validates_like_the_command() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let now = DbTime(Utc.with_ymd_and_hms(2026, 10, 20, 10, 0, 0).unwrap());
        let ok = check_absence(&input(None, "2026-10-25", " Vacances "), today, now).unwrap();
        assert_eq!(ok.motif, "Vacances");
        // Journées entières, heure de Paris (UTC+2 avant le 25/10, UTC+1 après).
        assert_eq!(ok.starts_at.0, Utc.with_ymd_and_hms(2026, 10, 19, 22, 0, 0).unwrap());
        assert_eq!(ok.ends_at.0, Utc.with_ymd_and_hms(2026, 10, 25, 22, 59, 0).unwrap());
        assert_eq!(check_absence(&input(None, "2026-10-25", "  "), today, now).unwrap_err(), "Indique un motif.");
        assert!(check_absence(&input(Some("2026-10-26"), "2026-10-25", "x"), today, now).unwrap_err().contains("fin"));
        assert!(
            check_absence(&input(Some("2026-10-01"), "2026-10-02", "x"), today, now).unwrap_err().contains("terminée")
        );
        assert_eq!(check_absence(&input(None, "25/10/2026", "x"), today, now).unwrap_err(), "Date invalide.");
        assert!(check_absence(&input(None, "2026-10-25", &"x".repeat(201)), today, now).is_err());
        // Un seul jour : aujourd'hui, de 00:00 à 23:59.
        assert!(check_absence(&input(None, "2026-10-20", "x"), today, now).is_ok());
    }
}
