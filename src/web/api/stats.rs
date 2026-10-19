//! Statistiques du serveur (`site.stats`), mêmes chiffres que la page.

use super::{
    ApiResult, MemberRef, Members, Session, iso_db, json,
    series::{self, paris_date},
    tickets::{TicketStats, ticket_stats},
};
use crate::{
    context::{Context, Ctx},
    discord::{members::role_member_ids, messages::parse_id},
    modules::{
        permissions::Perm,
        presences::{blocklist, model::ResponseStatus, report, service as presences},
        recrues::service::Entry as RecrueEntry,
    },
    util::time::{DbTime, paris_day_start},
    web::stats as page,
};
use anyhow::Result;
use axum::extract::{Path, State};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Serialize;
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};
use twilight_model::id::{Id, marker::GuildMarker};

/// Semaines du graphique d'assiduité.
const ATTENDANCE_WEEKS: i64 = 12;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Figure {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub hint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendance {
    pub member: MemberRef,
    pub present: i64,
    pub retard: i64,
    pub absent: i64,
    pub percent: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttendanceWeek {
    pub week_start: String,
    pub rate: Option<i64>,
    pub presences: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Veto {
    pub by: MemberRef,
    pub motif: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recruit {
    pub member: MemberRef,
    pub pour: usize,
    pub contre: usize,
    pub blanc: usize,
    pub veto: Option<Veto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAbsence {
    pub member: MemberRef,
    pub from: String,
    pub to: String,
    pub motif: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildStats {
    pub figures: Vec<Figure>,
    pub attendance: Vec<Attendance>,
    pub attendance_weekly: Vec<AttendanceWeek>,
    pub recruits: Vec<Recruit>,
    pub absences_current: Vec<CurrentAbsence>,
    pub tickets: Option<TicketStats>,
}

pub fn recruit(members: &Members, entry: &RecrueEntry) -> Recruit {
    let votes = |choix: &str| entry.votes.iter().filter(|v| v.choix == choix).count();
    Recruit {
        member: members.get(&entry.user_id),
        pour: votes("POUR"),
        contre: votes("CONTRE"),
        blanc: votes("BLANC"),
        veto: entry
            .veto_by_id
            .as_ref()
            .map(|by| Veto { by: members.get(by), motif: entry.veto_motif.clone().unwrap_or_default() }),
    }
}

/// Membre suivi par les présences : arrivée sur le serveur (attendu ensuite).
#[derive(Clone, Debug)]
pub struct Tracked {
    pub user_id: String,
    pub joined_at: Option<DbTime>,
}

/// Taux de présence par semaine, règle du rapport d'assiduité : présents ÷
/// attendus, les attendus d'une présence étant les membres suivis déjà arrivés
/// plus ceux qui ont répondu (hors blocklist). Un retard n'est pas présent.
pub fn attendance_weekly(
    weeks: &[NaiveDate],
    data: &report::WindowData,
    tracked: &[Tracked],
    blocklisted: &HashSet<String>,
) -> Vec<AttendanceWeek> {
    let mut by_presence: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    for response in &data.responses {
        if !blocklisted.contains(&response.user_id) {
            by_presence.entry(&response.presence_id).or_default().insert(&response.user_id, &response.status);
        }
    }
    weeks
        .iter()
        .map(|week| {
            let (start, end) = (paris_day_start(*week), paris_day_start(*week + Duration::weeks(1)));
            let (mut present, mut expected, mut count) = (0_i64, 0_i64, 0_i64);
            for presence in data.presences.iter().filter(|p| p.event_date >= start && p.event_date < end) {
                count += 1;
                let statuses = by_presence.get(presence.id.as_str());
                let mut attendees: HashSet<&str> = tracked
                    .iter()
                    .filter(|m| m.joined_at.is_none_or(|joined| joined <= presence.event_date))
                    .map(|m| m.user_id.as_str())
                    .collect();
                if let Some(statuses) = statuses {
                    attendees.extend(statuses.keys().copied());
                    present += statuses.values().filter(|s| **s == ResponseStatus::Present.as_str()).count() as i64;
                }
                expected += attendees.len() as i64;
            }
            AttendanceWeek {
                week_start: week.format("%Y-%m-%d").to_string(),
                rate: (expected > 0).then(|| ((present as f64) * 100.0 / expected as f64).round() as i64),
                presences: count,
            }
        })
        .collect()
}

/// Membres actuels du rôle des présences (cache), hors bots et blocklist.
fn tracked_members(ctx: &Ctx, guild_id: Id<GuildMarker>, role_id: &str, blocklisted: &HashSet<String>) -> Vec<Tracked> {
    let Some(role_id) = parse_id(role_id) else {
        return Vec::new();
    };
    role_member_ids(ctx, guild_id, role_id)
        .into_iter()
        .filter(|id| !blocklisted.contains(id))
        .filter_map(|id| {
            let user_id = parse_id(&id)?;
            if ctx.cache.user(user_id).is_some_and(|user| user.bot) {
                return None;
            }
            let joined_at = ctx
                .cache
                .member(guild_id, user_id)
                .and_then(|member| member.joined_at())
                .and_then(|joined| DateTime::from_timestamp_micros(joined.as_micros()))
                .map(DbTime);
            Some(Tracked { user_id: id, joined_at })
        })
        .collect()
}

async fn weekly_attendance(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<Vec<AttendanceWeek>> {
    let guild = guild_id.to_string();
    let weeks = series::last_weeks(paris_date(Utc::now()), ATTENDANCE_WEEKS);
    let (Some(first), Some(last)) = (weeks.first(), weeks.last()) else {
        return Ok(Vec::new());
    };
    let data =
        report::gather(&ctx.db, &guild, paris_day_start(*first), paris_day_start(*last + Duration::weeks(1))).await?;
    let blocklisted: HashSet<String> = blocklist::user_ids(&ctx.db, &guild).await?.into_iter().collect();
    let tracked = match presences::get_config(&ctx.db, &guild).await?.and_then(|c| c.default_role_id) {
        Some(role) => tracked_members(ctx, guild_id, &role, &blocklisted),
        None => Vec::new(),
    };
    Ok(attendance_weekly(&weeks, &data, &tracked, &blocklisted))
}

#[derive(FromRow)]
#[sqlx(rename_all = "camelCase")]
struct AbsenceRow {
    user_id: String,
    motif: String,
    starts_at: DbTime,
    ends_at: DbTime,
}

/// Absences en cours (mêmes conditions que le chiffre « Absences en cours »).
async fn current_absences(ctx: &Ctx, guild: &str) -> Result<Vec<AbsenceRow>> {
    Ok(sqlx::query_as(
        r#"SELECT "userId", "motif", "startsAt", "endsAt" FROM "Absence"
           WHERE "guildId" = ? AND "endedAt" IS NULL AND "startsAt" <= ? ORDER BY "endsAt" ASC"#,
    )
    .bind(guild)
    .bind(DbTime::now())
    .fetch_all(&ctx.db)
    .await?)
}

pub async fn stats(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require(&guild, Perm::SiteStats)?;
    let data = page::build(&ctx, guild_id).await?;
    let members = Members::new(&ctx, guild_id);
    let absences = current_absences(&ctx, &guild_id.to_string()).await?;
    let attendance_weekly = weekly_attendance(&ctx, guild_id).await?;
    Ok(json(&GuildStats {
        figures: data
            .figures
            .into_iter()
            .map(|f| Figure { key: f.key, label: f.label, value: f.value, hint: f.hint })
            .collect(),
        attendance: data
            .attendance
            .iter()
            .map(|e| Attendance {
                member: members.get(&e.user_id),
                present: e.present,
                retard: e.retard,
                absent: e.absent,
                percent: e.percent,
            })
            .collect(),
        attendance_weekly,
        recruits: data.recruits.iter().map(|entry| recruit(&members, entry)).collect(),
        absences_current: absences
            .into_iter()
            .map(|a| CurrentAbsence {
                member: members.get(&a.user_id),
                from: iso_db(a.starts_at),
                to: iso_db(a.ends_at),
                motif: a.motif,
            })
            .collect(),
        // Statistiques des tickets : seulement pour le staff des tickets.
        tickets: if session.access.tickets { data.tickets.map(ticket_stats) } else { None },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    fn at(d: u32, h: u32) -> DbTime {
        DbTime(Utc.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap())
    }

    #[test]
    fn weekly_rate_follows_the_report_rule() {
        let presence = |id: &str, date| report::ClosedPresence { id: id.into(), titre: id.into(), event_date: date };
        let response = |p: &str, u: &str, s: &str| report::ResponseRow {
            presence_id: p.into(),
            user_id: u.into(),
            status: s.into(),
        };
        let data = report::WindowData {
            // Semaine du 05/10 : deux présences ; semaine du 12/10 : aucune.
            presences: vec![presence("p1", at(6, 19)), presence("p2", at(8, 19))],
            responses: vec![
                response("p1", "a", "PRESENT"),
                response("p1", "b", "RETARD"),
                response("p2", "a", "PRESENT"),
                response("p2", "x", "PRESENT"),
                response("p2", "banni", "PRESENT"),
            ],
            absences: Vec::new(),
        };
        let tracked = vec![
            Tracked { user_id: "a".into(), joined_at: None },
            Tracked { user_id: "b".into(), joined_at: None },
            // Arrivé après la première présence : attendu seulement à la seconde.
            Tracked { user_id: "c".into(), joined_at: Some(at(7, 0)) },
        ];
        let blocklisted: HashSet<String> = ["banni".to_owned()].into();
        let weeks = vec![day("2026-10-05"), day("2026-10-12")];
        let result = attendance_weekly(&weeks, &data, &tracked, &blocklisted);
        // p1 : 1 présent / 2 attendus (a, b) ; p2 : 2 présents / 4 attendus (a, b, c, x).
        assert_eq!(
            result,
            vec![
                AttendanceWeek { week_start: "2026-10-05".into(), rate: Some(50), presences: 2 },
                AttendanceWeek { week_start: "2026-10-12".into(), rate: None, presences: 0 },
            ]
        );
    }
}
