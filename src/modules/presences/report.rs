//! Rapport hebdomadaire d'assiduité, publié le lundi à partir de 09:00 (heure
//! de Paris) dans le salon `reportChannelId`.
//!
//! - Semaine couverte : du lundi au dimanche précédents, heure de Paris.
//! - Envoi au plus une fois par semaine et par guilde : `lastReportAt` est
//!   réservé par un UPDATE conditionnel avant l'envoi (comme le rappel des
//!   recrues). Un bot arrêté à 09:00 rattrape l'envoi plus tard le même lundi ;
//!   au-delà, la semaine est sautée.
//! - Seules les présences clôturées comptent, comme dans `/presence stats`.
//! - Taux de présence = présents ÷ attendus. Un retard n'est pas compté présent
//!   (même règle que `/presence stats`), il est affiché à part. Les attendus d'une
//!   présence sont les membres actuels du rôle des présences (hors bots et
//!   blocklist) déjà arrivés sur le serveur à la date de l'événement, plus ceux
//!   qui y ont répondu sans (ou plus) porter le rôle.
//! - Section « Sous le seuil » : membres actuels du rôle dont le taux sur les
//!   4 dernières semaines est inférieur au seuil. Un membre dont les absences
//!   déclarées couvrent plus de la moitié des présences de la période n'est pas
//!   exclu mais marqué « (absence déclarée) » : le staff garde l'information et
//!   voit que l'absence était prévenue.

use super::{
    blocklist,
    model::{COLOR_ACTIVE, ResponseStatus},
    service,
};
use crate::{
    context::Ctx,
    discord::{
        components::{container, separator, text_displays},
        members::role_member_ids,
        messages::{self, Outgoing, parse_id},
    },
    util::{
        format::{V2_TEXT_LIMIT, and_others, bounded_lines, mention, plural, truncate},
        time::{DbTime, TIMEZONE, paris_day_start, week_start},
    },
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc, Weekday};
use sqlx::{FromRow, SqlitePool};
use std::collections::{HashMap, HashSet};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::GuildMarker},
};

/// Heure locale (Paris) à partir de laquelle le rapport part, le lundi.
pub const REPORT_HOUR: u32 = 9;
/// Nombre de semaines prises en compte pour la section « Sous le seuil ».
pub const ALERT_WEEKS: i64 = 4;
/// Présences listées au plus avant « … et N autres ».
const MAX_PRESENCE_LINES: usize = 10;
const TITLE_LIMIT: usize = 60;
const PRESENCES_BUDGET: usize = 1600;
const TOP_COUNT: usize = 3;

/// Lundi à partir de 09:00, heure de Paris. Vérifié à chaque passage du cron
/// jusqu'à minuit : un redémarrage dans la matinée ne fait pas manquer l'envoi.
pub fn report_due(now: DateTime<Utc>) -> bool {
    let local = now.with_timezone(&TIMEZONE);
    local.weekday() == Weekday::Mon && local.hour() >= REPORT_HOUR
}

/// Bornes du rapport (instants UTC correspondant à minuit heure de Paris).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportWindow {
    /// Début de la fenêtre d'alerte (4 semaines avant `end`).
    pub alert_start: DbTime,
    /// Début de la semaine précédant la semaine du rapport (comparaison).
    pub previous_start: DbTime,
    /// Lundi 00:00 de la semaine du rapport.
    pub start: DbTime,
    /// Lundi 00:00 suivant (exclu) : début de la semaine en cours.
    pub end: DbTime,
    /// Premier et dernier jour (lundi, dimanche) de la semaine du rapport.
    pub first_day: NaiveDate,
    pub last_day: NaiveDate,
}

/// Semaine écoulée (lundi → dimanche, heure de Paris) par rapport à `now`.
pub fn report_window(now: DateTime<Utc>) -> ReportWindow {
    let monday = week_start(now).0.with_timezone(&TIMEZONE).date_naive();
    let day = |weeks_back: i64| monday - Duration::weeks(weeks_back);
    ReportWindow {
        alert_start: paris_day_start(day(ALERT_WEEKS)),
        previous_start: paris_day_start(day(2)),
        start: paris_day_start(day(1)),
        end: paris_day_start(monday),
        first_day: day(1),
        last_day: monday - Duration::days(1),
    }
}

#[derive(Clone, Debug, FromRow, PartialEq)]
#[sqlx(rename_all = "camelCase")]
pub struct ClosedPresence {
    pub id: String,
    pub titre: String,
    pub event_date: DbTime,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
#[sqlx(rename_all = "camelCase")]
pub struct ResponseRow {
    pub presence_id: String,
    pub user_id: String,
    pub status: String,
}

/// Période d'absence déclarée effective (fin anticipée prise en compte).
#[derive(Clone, Debug, FromRow, PartialEq)]
#[sqlx(rename_all = "camelCase")]
pub struct AbsenceSpan {
    pub user_id: String,
    pub starts_at: DbTime,
    pub ends_at: DbTime,
}

/// Données brutes de la fenêtre d'alerte (4 semaines).
#[derive(Clone, Debug, Default)]
pub struct WindowData {
    /// Présences clôturées de la fenêtre, par date croissante.
    pub presences: Vec<ClosedPresence>,
    pub responses: Vec<ResponseRow>,
    pub absences: Vec<AbsenceSpan>,
}

/// Rassemble les présences clôturées dont l'événement tombe dans
/// `[from, to[`, leurs réponses et les absences déclarées qui chevauchent la
/// période. Une absence annulée avant son début a `endedAt < startsAt` : elle
/// est écartée.
pub async fn gather(db: &SqlitePool, guild_id: &str, from: DbTime, to: DbTime) -> Result<WindowData> {
    let presences = sqlx::query_as(
        r#"SELECT "id", "titre", "eventDate" FROM "Presence"
           WHERE "guildId" = ? AND "closedAt" IS NOT NULL AND "eventDate" >= ? AND "eventDate" < ?
           ORDER BY "eventDate" ASC, "id" ASC"#,
    )
    .bind(guild_id)
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?;
    let responses = sqlx::query_as(
        r#"SELECT r."presenceId", r."userId", r."status"
           FROM "PresenceResponse" r JOIN "Presence" p ON p."id" = r."presenceId"
           WHERE p."guildId" = ? AND p."closedAt" IS NOT NULL AND p."eventDate" >= ? AND p."eventDate" < ?
           ORDER BY r."presenceId", r."userId""#,
    )
    .bind(guild_id)
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await?;
    let absences = sqlx::query_as(
        r#"SELECT "userId", "startsAt", MIN("endsAt", COALESCE("endedAt", "endsAt")) AS "endsAt"
           FROM "Absence"
           WHERE "guildId" = ? AND "startsAt" < ? AND MIN("endsAt", COALESCE("endedAt", "endsAt")) > ?
             AND MIN("endsAt", COALESCE("endedAt", "endsAt")) > "startsAt"
           ORDER BY "userId", "startsAt""#,
    )
    .bind(guild_id)
    .bind(to)
    .bind(from)
    .fetch_all(db)
    .await?;
    Ok(WindowData { presences, responses, absences })
}

/// Membre actuel du rôle des présences (hors bots et blocklist).
#[derive(Clone, Debug, PartialEq)]
pub struct TrackedMember {
    pub user_id: String,
    /// Arrivée sur le serveur : les présences antérieures ne lui sont pas comptées.
    pub joined_at: Option<DbTime>,
}

impl TrackedMember {
    fn expected_at(&self, date: DbTime) -> bool {
        self.joined_at.is_none_or(|joined| joined <= date)
    }
}

/// Présents sur attendus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rate {
    pub present: i64,
    pub expected: i64,
}

impl Rate {
    pub fn percent(self) -> Option<f64> {
        (self.expected > 0).then(|| self.present as f64 * 100.0 / self.expected as f64)
    }

    pub fn rounded(self) -> Option<i64> {
        self.percent().map(|value| value.round() as i64)
    }

    fn add(&mut self, other: Rate) {
        self.present += other.present;
        self.expected += other.expected;
    }
}

/// Écart en points entre deux taux, arrondi (`None` sans base de comparaison).
pub fn delta_points(current: Rate, previous: Rate) -> Option<i64> {
    Some((current.percent()? - previous.percent()?).round() as i64)
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresenceLine {
    pub titre: String,
    pub event_date: DbTime,
    pub present: i64,
    pub absent: i64,
    pub retard: i64,
    pub no_answer: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopEntry {
    pub user_id: String,
    pub present: i64,
    pub expected: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlertEntry {
    pub user_id: String,
    pub rate: Rate,
    pub absent: i64,
    pub retard: i64,
    pub no_answer: i64,
    pub declared_absence: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub first_day: NaiveDate,
    pub last_day: NaiveDate,
    pub presences: Vec<PresenceLine>,
    pub rate: Rate,
    /// Écart avec la semaine précédente, si elle avait des présences clôturées.
    pub delta: Option<i64>,
    pub top: Vec<TopEntry>,
    pub threshold: i64,
    /// `None` : rôle des présences non configuré.
    pub alerts: Option<Vec<AlertEntry>>,
}

/// Statistiques d'une présence : réponses par membre et attendus.
struct PresenceStats<'a> {
    presence: &'a ClosedPresence,
    statuses: HashMap<&'a str, ResponseStatus>,
    expected: HashSet<&'a str>,
}

impl PresenceStats<'_> {
    fn count(&self, status: ResponseStatus) -> i64 {
        self.statuses.values().filter(|s| **s == status).count() as i64
    }

    fn rate(&self) -> Rate {
        Rate { present: self.count(ResponseStatus::Present), expected: self.expected.len() as i64 }
    }

    fn line(&self) -> PresenceLine {
        PresenceLine {
            titre: self.presence.titre.clone(),
            event_date: self.presence.event_date,
            present: self.count(ResponseStatus::Present),
            absent: self.count(ResponseStatus::Absent),
            retard: self.count(ResponseStatus::Retard),
            no_answer: self.expected.iter().filter(|id| !self.statuses.contains_key(*id)).count() as i64,
        }
    }
}

/// Construit le rapport. `None` s'il n'y a eu aucune présence clôturée sur la
/// fenêtre d'alerte : rien d'utile à dire, le rapport est sauté.
pub fn build_report(
    window: &ReportWindow,
    data: &WindowData,
    members: Option<&[TrackedMember]>,
    blocklisted: &HashSet<String>,
    threshold: i64,
) -> Option<Report> {
    let in_window = |p: &&ClosedPresence| p.event_date >= window.alert_start && p.event_date < window.end;
    let presences: Vec<&ClosedPresence> = data.presences.iter().filter(in_window).collect();
    if presences.is_empty() {
        return None;
    }
    let tracked = members.unwrap_or_default();

    let mut by_presence: HashMap<&str, HashMap<&str, ResponseStatus>> = HashMap::new();
    for response in &data.responses {
        if blocklisted.contains(&response.user_id) {
            continue;
        }
        let status = ResponseStatus::parse(&response.status).unwrap_or(ResponseStatus::Absent);
        by_presence.entry(&response.presence_id).or_default().insert(&response.user_id, status);
    }

    let stats: Vec<PresenceStats> = presences
        .iter()
        .map(|&presence| {
            let statuses = by_presence.remove(presence.id.as_str()).unwrap_or_default();
            let mut expected: HashSet<&str> =
                tracked.iter().filter(|m| m.expected_at(presence.event_date)).map(|m| m.user_id.as_str()).collect();
            expected.extend(statuses.keys().copied());
            PresenceStats { presence, statuses, expected }
        })
        .collect();

    let between = |from: DbTime, to: DbTime| {
        stats.iter().filter(move |s| s.presence.event_date >= from && s.presence.event_date < to)
    };
    let week: Vec<&PresenceStats> = between(window.start, window.end).collect();

    let mut rate = Rate::default();
    week.iter().for_each(|s| rate.add(s.rate()));
    let mut previous = Rate::default();
    between(window.previous_start, window.start).for_each(|s| previous.add(s.rate()));

    let mut top: Vec<TopEntry> = week
        .iter()
        .flat_map(|s| s.expected.iter().copied())
        .collect::<HashSet<&str>>()
        .into_iter()
        .map(|user_id| TopEntry {
            user_id: user_id.to_owned(),
            present: week.iter().filter(|s| s.statuses.get(user_id) == Some(&ResponseStatus::Present)).count() as i64,
            expected: week.iter().filter(|s| s.expected.contains(user_id)).count() as i64,
        })
        .filter(|entry| entry.present > 0)
        .collect();
    top.sort_by(|a, b| b.present.cmp(&a.present).then(a.expected.cmp(&b.expected)).then(a.user_id.cmp(&b.user_id)));
    top.truncate(TOP_COUNT);

    let alerts = members.map(|members| alert_entries(&stats, members, &data.absences, threshold));

    Some(Report {
        first_day: window.first_day,
        last_day: window.last_day,
        presences: week.iter().map(|s| s.line()).collect(),
        rate,
        delta: if previous.expected > 0 { delta_points(rate, previous) } else { None },
        top,
        threshold,
        alerts,
    })
}

/// Membres sous le seuil sur la fenêtre d'alerte, du plus bas taux au plus haut.
fn alert_entries(
    stats: &[PresenceStats],
    members: &[TrackedMember],
    absences: &[AbsenceSpan],
    threshold: i64,
) -> Vec<AlertEntry> {
    let mut entries: Vec<AlertEntry> = members
        .iter()
        .filter_map(|member| {
            let id = member.user_id.as_str();
            let expected: Vec<&PresenceStats> = stats.iter().filter(|s| s.expected.contains(id)).collect();
            let count = |status| expected.iter().filter(|s| s.statuses.get(id) == Some(&status)).count() as i64;
            let rate = Rate { present: count(ResponseStatus::Present), expected: expected.len() as i64 };
            // Comparaison exacte : 49,6 % reste sous un seuil de 50 %.
            if rate.expected == 0 || rate.present * 100 >= threshold * rate.expected {
                return None;
            }
            let covered = expected
                .iter()
                .filter(|s| {
                    let date = s.presence.event_date;
                    absences.iter().any(|a| a.user_id == id && a.starts_at <= date && date < a.ends_at)
                })
                .count() as i64;
            Some(AlertEntry {
                user_id: member.user_id.clone(),
                rate,
                absent: count(ResponseStatus::Absent),
                retard: count(ResponseStatus::Retard),
                no_answer: expected.iter().filter(|s| !s.statuses.contains_key(id)).count() as i64,
                declared_absence: covered * 2 > rate.expected,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        // a.present / a.expected < b.present / b.expected, sans flottants.
        (a.rate.present * b.rate.expected)
            .cmp(&(b.rate.present * a.rate.expected))
            .then(b.rate.expected.cmp(&a.rate.expected))
            .then(a.user_id.cmp(&b.user_id))
    });
    entries
}

fn percent_label(rate: Rate) -> String {
    rate.rounded().map_or_else(|| "—".to_owned(), |value| format!("{value} %"))
}

fn delta_label(delta: i64) -> String {
    match delta {
        0 => "= stable par rapport à la semaine précédente".to_owned(),
        d if d > 0 => format!("▲ {d} pt{} par rapport à la semaine précédente", plural(d as usize)),
        d => format!("▼ {} pt{} par rapport à la semaine précédente", -d, plural((-d) as usize)),
    }
}

/// Textes du rapport, dans l'ordre d'affichage, sous la limite de 4000 octets.
pub fn report_texts(report: &Report) -> Vec<String> {
    let mut header = format!(
        "## 📊 Assiduité — semaine du {} au {}\n",
        report.first_day.format("%d/%m"),
        report.last_day.format("%d/%m")
    );
    let count = report.presences.len();
    if count == 0 {
        header.push_str("Aucune présence clôturée cette semaine.");
    } else {
        header.push_str(&format!(
            "**{count}** présence{} clôturée{} · taux de présence **{}**",
            plural(count),
            plural(count),
            percent_label(report.rate)
        ));
        if let Some(delta) = report.delta {
            header.push_str(&format!("\n{}", delta_label(delta)));
        }
    }
    let mut texts = vec![header];

    if count > 0 {
        let lines: Vec<String> = report
            .presences
            .iter()
            .map(|p| {
                format!(
                    "- **{}** <t:{}:d> · ✅ {} · ❌ {} · ⏳ {} · ❔ {}",
                    truncate(&p.titre.replace(['\n', '*'], " "), TITLE_LIMIT),
                    p.event_date.unix(),
                    p.present,
                    p.absent,
                    p.retard,
                    p.no_answer
                )
            })
            .collect();
        let hidden = lines.len().saturating_sub(MAX_PRESENCE_LINES);
        let shown = &lines[..lines.len().min(MAX_PRESENCE_LINES)];
        let mut body = bounded_lines(shown, PRESENCES_BUDGET, |n| and_others(n + hidden));
        let dropped_by_budget = shown.len() - body.lines().filter(|l| l.starts_with("- ")).count();
        if hidden > 0 && dropped_by_budget == 0 {
            body.push('\n');
            body.push_str(&and_others(hidden));
        }
        texts.push(format!("### 📋 Présences\n{body}\n-# ✅ présent · ❌ absent · ⏳ retard · ❔ sans réponse"));

        if !report.top.is_empty() {
            let medals = ["🥇", "🥈", "🥉"];
            let lines: Vec<String> = report
                .top
                .iter()
                .zip(medals)
                .map(|(entry, medal)| {
                    format!("{medal} {} — {}/{}", mention(&entry.user_id), entry.present, entry.expected)
                })
                .collect();
            texts.push(format!("### 🏆 Les plus présents\n{}", lines.join("\n")));
        }
    }

    let alert_header = format!("### ⚠️ Sous le seuil ({} % sur {ALERT_WEEKS} semaines)", report.threshold);
    let used: usize = texts.iter().map(String::len).sum::<usize>() + alert_header.len() + 1;
    let alert_body = match &report.alerts {
        None => "*Rôle des présences non configuré.*".to_owned(),
        Some(alerts) if alerts.is_empty() => "*Personne sous le seuil.*".to_owned(),
        Some(alerts) => {
            let lines: Vec<String> = alerts
                .iter()
                .map(|a| {
                    format!(
                        "- {} — **{}** ({}/{} · ❌ {} · ⏳ {} · ❔ {}){}",
                        mention(&a.user_id),
                        percent_label(a.rate),
                        a.rate.present,
                        a.rate.expected,
                        a.absent,
                        a.retard,
                        a.no_answer,
                        if a.declared_absence { " *(absence déclarée)*" } else { "" }
                    )
                })
                .collect();
            bounded_lines(&lines, V2_TEXT_LIMIT.saturating_sub(used), and_others)
        }
    };
    texts.push(format!("{alert_header}\n{alert_body}"));
    texts
}

/// Message Components V2 du rapport : un encadré, sections séparées.
pub fn report_components(report: &Report) -> Vec<Component> {
    let mut children = Vec::new();
    for (index, text) in report_texts(report).iter().enumerate() {
        if index > 0 {
            children.push(separator());
        }
        children.extend(text_displays(text));
    }
    vec![container(Some(COLOR_ACTIVE), children)]
}

/// Réserve l'envoi de la semaine : `true` si cet appel l'a obtenu.
pub async fn claim_report(db: &SqlitePool, guild_id: &str, week_start: DbTime) -> Result<bool> {
    let result = sqlx::query(
        r#"UPDATE "PresenceConfig" SET "lastReportAt" = ?
           WHERE "guildId" = ? AND "reportChannelId" IS NOT NULL AND ("lastReportAt" IS NULL OR "lastReportAt" < ?)"#,
    )
    .bind(DbTime::now())
    .bind(guild_id)
    .bind(week_start)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Guildes dont le rapport de la semaine n'est pas encore parti.
pub async fn pending_guilds(db: &SqlitePool, week_start: DbTime) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "guildId" FROM "PresenceConfig"
           WHERE "reportChannelId" IS NOT NULL AND ("lastReportAt" IS NULL OR "lastReportAt" < ?)"#,
    )
    .bind(week_start)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Membres actuels du rôle des présences, hors bots et blocklist (cache).
fn tracked_members(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    role_id: &str,
    blocklisted: &HashSet<String>,
) -> Vec<TrackedMember> {
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
            Some(TrackedMember { user_id: id, joined_at })
        })
        .collect()
}

/// Envoie le rapport de la semaine écoulée pour une guilde, s'il est dû.
pub async fn send_report(ctx: &Ctx, guild_id: Id<GuildMarker>, now: DateTime<Utc>) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(config) = service::get_config(&ctx.db, &guild).await? else {
        return Ok(());
    };
    let Some(channel_id) = config.report_channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    // Cache des membres pas encore rempli (redémarrage) : on retentera au passage suivant.
    if ctx.cache.guild_members(guild_id).is_none_or(|members| members.is_empty()) {
        return Ok(());
    }

    let window = report_window(now);
    // Réservé avant l'envoi : au plus un rapport par semaine, même s'il est sauté.
    if !claim_report(&ctx.db, &guild, window.end).await? {
        return Ok(());
    }

    let blocklisted: HashSet<String> = blocklist::user_ids(&ctx.db, &guild).await?.into_iter().collect();
    let members = config.default_role_id.as_deref().map(|role| tracked_members(ctx, guild_id, role, &blocklisted));
    let data = gather(&ctx.db, &guild, window.alert_start, window.end).await?;
    let Some(report) = build_report(&window, &data, members.as_deref(), &blocklisted, config.alert_threshold()) else {
        tracing::info!(%guild_id, "rapport d'assiduité sauté : aucune présence sur 4 semaines");
        return Ok(());
    };

    // Mentions vides par défaut : les membres cités ne sont pas pingés.
    messages::send(ctx, channel_id, &Outgoing::components_v2(report_components(&report))).await?;
    tracing::info!(%guild_id, presences = report.presences.len(), "rapport d'assiduité envoyé");
    Ok(())
}

/// Passage du cron : envoie les rapports dus.
pub async fn sweep(ctx: &Ctx) {
    let now = Utc::now();
    if !report_due(now) {
        return;
    }
    let guilds = match pending_guilds(&ctx.db, week_start(now)).await {
        Ok(guilds) => guilds,
        Err(error) => return tracing::error!(?error, "lecture des rapports d'assiduité à envoyer"),
    };
    for guild in guilds {
        let Some(guild_id) = parse_id(&guild) else { continue };
        if let Err(error) = send_report(ctx, guild_id, now).await {
            tracing::error!(%guild_id, ?error, "rapport d'assiduité échoué");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        util::format::V2_COMPONENT_LIMIT,
    };
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    fn at(y: i32, m: u32, d: u32, h: u32) -> DbTime {
        DbTime(utc(y, m, d, h, 0))
    }

    fn presence(id: &str, date: DbTime) -> ClosedPresence {
        ClosedPresence { id: id.to_owned(), titre: format!("Soirée {id}"), event_date: date }
    }

    fn response(presence_id: &str, user_id: &str, status: ResponseStatus) -> ResponseRow {
        ResponseRow {
            presence_id: presence_id.to_owned(),
            user_id: user_id.to_owned(),
            status: status.as_str().to_owned(),
        }
    }

    fn member(id: &str) -> TrackedMember {
        TrackedMember { user_id: id.to_owned(), joined_at: None }
    }

    #[test]
    fn due_on_paris_monday_morning() {
        // Été (UTC+2) : lundi 07:00 UTC = 09:00 à Paris.
        assert!(report_due(utc(2026, 8, 17, 7, 0)));
        assert!(!report_due(utc(2026, 8, 17, 6, 59)));
        assert!(report_due(utc(2026, 8, 17, 21, 59)));
        // Lundi 22:00 UTC = mardi 00:00 à Paris : trop tard, la semaine est sautée.
        assert!(!report_due(utc(2026, 8, 17, 22, 0)));
        // Hiver (UTC+1) : lundi 08:00 UTC = 09:00.
        assert!(report_due(utc(2026, 10, 26, 8, 0)));
        assert!(!report_due(utc(2026, 10, 26, 7, 59)));
        // Dimanche soir.
        assert!(!report_due(utc(2026, 10, 25, 20, 0)));
    }

    #[test]
    fn window_covers_previous_paris_week() {
        let window = report_window(utc(2026, 8, 17, 7, 0));
        assert_eq!(window.start.to_db_string(), "2026-08-09T22:00:00.000Z");
        assert_eq!(window.end.to_db_string(), "2026-08-16T22:00:00.000Z");
        assert_eq!(window.previous_start.to_db_string(), "2026-08-02T22:00:00.000Z");
        assert_eq!(window.alert_start.to_db_string(), "2026-07-19T22:00:00.000Z");
        assert_eq!(window.first_day, NaiveDate::from_ymd_opt(2026, 8, 10).unwrap());
        assert_eq!(window.last_day, NaiveDate::from_ymd_opt(2026, 8, 16).unwrap());
        // Même résultat plus tard dans la journée (rattrapage).
        assert_eq!(report_window(utc(2026, 8, 17, 20, 0)), window);
    }

    #[test]
    fn window_handles_dst_changes() {
        // Semaine du passage à l'heure d'hiver (dimanche 25/10/2026) : 169 h.
        let autumn = report_window(utc(2026, 10, 26, 8, 0));
        assert_eq!(autumn.start.to_db_string(), "2026-10-18T22:00:00.000Z");
        assert_eq!(autumn.end.to_db_string(), "2026-10-25T23:00:00.000Z");
        assert_eq!(autumn.end.0 - autumn.start.0, Duration::hours(169));
        assert_eq!(autumn.alert_start.to_db_string(), "2026-09-27T22:00:00.000Z");
        // Semaine du passage à l'heure d'été (dimanche 29/03/2026) : 167 h.
        let spring = report_window(utc(2026, 3, 30, 7, 0));
        assert_eq!(spring.start.to_db_string(), "2026-03-22T23:00:00.000Z");
        assert_eq!(spring.end.to_db_string(), "2026-03-29T22:00:00.000Z");
        assert_eq!(spring.end.0 - spring.start.0, Duration::hours(167));
        assert_eq!(spring.last_day, NaiveDate::from_ymd_opt(2026, 3, 29).unwrap());
    }

    /// Semaine du 10 au 16/08/2026 : p1, p2 ; semaine précédente : p0 ;
    /// plus ancienne dans la fenêtre : pm1 ; hors fenêtre : old.
    fn fixture() -> (ReportWindow, WindowData, Vec<TrackedMember>, HashSet<String>) {
        use ResponseStatus::{Absent as A, Present as P, Retard as R};
        let window = report_window(utc(2026, 8, 17, 8, 0));
        let data = WindowData {
            presences: vec![
                presence("old", at(2026, 7, 10, 19)),
                presence("pm1", at(2026, 7, 25, 19)),
                presence("p0", at(2026, 8, 5, 19)),
                presence("p1", at(2026, 8, 12, 19)),
                presence("p2", at(2026, 8, 14, 19)),
            ],
            responses: vec![
                response("old", "c", A),
                response("pm1", "a", P),
                response("pm1", "c", A),
                response("p0", "a", P),
                response("p0", "b", P),
                response("p0", "c", P),
                response("p1", "a", P),
                response("p1", "b", A),
                response("p1", "c", R),
                response("p1", "z", A),
                response("p2", "a", P),
                response("p2", "b", P),
                response("p2", "d", P),
            ],
            absences: vec![AbsenceSpan {
                user_id: "c".to_owned(),
                starts_at: at(2026, 7, 24, 0),
                ends_at: at(2026, 8, 13, 0),
            }],
        };
        let mut members = vec![member("a"), member("b"), member("c"), member("d")];
        // « d » est arrivé le 13/08 : seule p2 lui est comptée.
        members[3].joined_at = Some(at(2026, 8, 13, 12));
        let blocklisted = HashSet::from(["z".to_owned()]);
        (window, data, members, blocklisted)
    }

    #[test]
    fn computes_week_rates_and_comparison() {
        let (window, data, members, blocklisted) = fixture();
        let report = build_report(&window, &data, Some(&members), &blocklisted, 50).unwrap();
        assert_eq!(report.presences.len(), 2);
        // p1 : a, b, c attendus (d pas encore arrivé, z blocklisté) ; p2 : a, b, c, d.
        let p1 = &report.presences[0];
        assert_eq!((p1.present, p1.absent, p1.retard, p1.no_answer), (1, 1, 1, 0));
        assert_eq!((report.presences[1].present, report.presences[1].no_answer), (3, 1));
        assert_eq!(report.rate, Rate { present: 4, expected: 7 });
        assert_eq!(report.rate.rounded(), Some(57));
        // Semaine précédente : 3/3 = 100 % → 57,1 − 100 = −42,9.
        assert_eq!(report.delta, Some(-43));
        let top: Vec<(&str, i64, i64)> =
            report.top.iter().map(|t| (t.user_id.as_str(), t.present, t.expected)).collect();
        assert_eq!(top, vec![("a", 2, 2), ("d", 1, 1), ("b", 1, 2)]);
    }

    #[test]
    fn filters_members_under_threshold() {
        let (window, data, members, blocklisted) = fixture();
        let report = build_report(&window, &data, Some(&members), &blocklisted, 50).unwrap();
        // b : 2/4 = 50 %, pile au seuil, n'est pas signalé ; c : 1/4.
        let alerts = report.alerts.unwrap();
        assert_eq!(alerts.len(), 1);
        let c = &alerts[0];
        assert_eq!(c.user_id, "c");
        assert_eq!((c.rate, c.absent, c.retard, c.no_answer), (Rate { present: 1, expected: 4 }, 1, 1, 1));
        // Absence déclarée couvrant pm1, p0 et p1 : 3 présences sur 4.
        assert!(c.declared_absence);

        let report = build_report(&window, &data, Some(&members), &blocklisted, 60).unwrap();
        let ids: Vec<String> = report.alerts.unwrap().into_iter().map(|a| a.user_id).collect();
        assert_eq!(ids, vec!["c", "b"]);

        // Absence courte (pm1 seulement) : pas de mention.
        let mut short = data.clone();
        short.absences[0].ends_at = at(2026, 7, 26, 0);
        let report = build_report(&window, &short, Some(&members), &blocklisted, 50).unwrap();
        assert!(!report.alerts.unwrap()[0].declared_absence);
    }

    #[test]
    fn rates_and_deltas() {
        assert_eq!(Rate::default().percent(), None);
        assert_eq!(Rate { present: 1, expected: 3 }.rounded(), Some(33));
        assert_eq!(Rate { present: 2, expected: 3 }.rounded(), Some(67));
        let half = Rate { present: 1, expected: 2 };
        assert_eq!(delta_points(Rate { present: 3, expected: 4 }, half), Some(25));
        assert_eq!(delta_points(half, Rate { present: 3, expected: 4 }), Some(-25));
        assert_eq!(delta_points(half, Rate::default()), None);
        assert_eq!(delta_label(5), "▲ 5 pts par rapport à la semaine précédente");
        assert_eq!(delta_label(-1), "▼ 1 pt par rapport à la semaine précédente");
        assert!(delta_label(0).starts_with("= stable"));
    }

    #[test]
    fn renders_full_report() {
        let (window, data, members, blocklisted) = fixture();
        let report = build_report(&window, &data, Some(&members), &blocklisted, 50).unwrap();
        let components = report_components(&report);
        let text = v2_text(&components);
        assert!(text.starts_with("## 📊 Assiduité — semaine du 10/08 au 16/08\n"));
        assert!(text.contains("**2** présences clôturées · taux de présence **57 %**"));
        assert!(text.contains("▼ 43 pts"));
        let p1 = format!("- **Soirée p1** <t:{}:d> · ✅ 1 · ❌ 1 · ⏳ 1 · ❔ 0", at(2026, 8, 12, 19).unix());
        assert!(text.contains(&p1));
        assert!(text.contains("🥇 <@a> — 2/2\n🥈 <@d> — 1/1\n🥉 <@b> — 1/2"));
        assert!(text.contains(
            "### ⚠️ Sous le seuil (50 % sur 4 semaines)\n- <@c> — **25 %** (1/4 · ❌ 1 · ⏳ 1 · ❔ 1) *(absence déclarée)*"
        ));
        // Jamais de ping : aucune mention autorisée.
        let message = Outgoing::components_v2(components);
        assert!(message.is_v2());
        let allowed = message.mentions.to_allowed();
        assert!(allowed.parse.is_empty() && allowed.users.is_empty() && allowed.roles.is_empty());
    }

    #[test]
    fn empty_week_and_skipped_report() {
        let (window, mut data, members, blocklisted) = fixture();
        data.presences.retain(|p| !["p1", "p2"].contains(&p.id.as_str()));
        let report = build_report(&window, &data, Some(&members), &blocklisted, 50).unwrap();
        assert!(report.presences.is_empty() && report.top.is_empty() && report.delta.is_none());
        let text = report_texts(&report).join("\n");
        assert!(text.contains("Aucune présence clôturée cette semaine."));
        assert!(!text.contains("### 📋") && !text.contains("taux de présence"));
        assert!(text.contains("Sous le seuil"));

        // Rien sur 4 semaines (« old » est hors fenêtre) : rapport sauté.
        data.presences.retain(|p| p.id == "old");
        assert!(build_report(&window, &data, Some(&members), &blocklisted, 50).is_none());

        // Rôle des présences non configuré.
        let (window, data, ..) = fixture();
        let report = build_report(&window, &data, None, &blocklisted, 50).unwrap();
        assert!(report.alerts.is_none());
        assert!(report_texts(&report).last().unwrap().ends_with("*Rôle des présences non configuré.*"));
    }

    #[test]
    fn truncates_long_reports() {
        let window = report_window(utc(2026, 8, 17, 8, 0));
        let members: Vec<TrackedMember> =
            (0..300).map(|i| member(&format!("{}", 100_000_000_000_000_000u64 + i))).collect();
        let presences: Vec<ClosedPresence> = (0..15)
            .map(|i| ClosedPresence {
                id: format!("p{i}"),
                titre: "Très long titre de présence ".repeat(5),
                event_date: DbTime(window.start.0 + Duration::hours(i * 10)),
            })
            .collect();
        let data = WindowData { presences, ..Default::default() };
        let report = build_report(&window, &data, Some(&members), &HashSet::new(), 50).unwrap();
        assert_eq!(report.presences.len(), 15);
        assert_eq!(report.alerts.as_ref().unwrap().len(), 300);

        let texts = report_texts(&report);
        assert_eq!(texts[1].lines().filter(|l| l.starts_with("- ")).count(), 10);
        assert!(texts[1].contains("\n… et 5 autres\n"));
        assert!(texts.last().unwrap().contains(" autres"));

        let components = report_components(&report);
        let (bytes, count) = v2_usage(&components);
        assert!(bytes <= V2_TEXT_LIMIT, "{bytes} octets");
        assert!(count <= V2_COMPONENT_LIMIT, "{count} composants");
    }
}
