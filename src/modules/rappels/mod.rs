//! Rappels personnels en message privé, désactivés par défaut (`/config rappels`) :
//!
//! - **présence** : quand une présence ouverte commence dans moins de N heures
//!   (2 par défaut), un MP à chaque membre attendu qui n'a pas répondu (mêmes
//!   règles que les relances : hors absents d'office et blocklist), une seule
//!   fois par présence. Une présence tout juste publiée (elle pingue déjà) laisse
//!   30 minutes avant le premier MP ;
//! - **objectif de cotisation** : le samedi à partir de 18:00 (heure de Paris),
//!   un MP aux membres suivis encore en dessous de l'objectif (hors exemptés),
//!   avec le reste à verser, une fois par semaine.
//!
//! Chaque membre coupe les siens avec `/rappels`. Un MP refusé (messages privés
//! fermés) est ignoré et pas retenté avant 7 jours. Chaque envoi est noté en
//! base avant de partir : un redémarrage ne renvoie rien.

pub mod commands;
pub mod cron;
pub mod service;

use crate::{
    modules::cotisations::{
        money,
        objective::{Objective, ProgressRow},
    },
    util::time::{DbTime, TIMEZONE},
};
use chrono::{DateTime, Datelike, Duration, Timelike, Utc, Weekday};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_PRESENCE_HOURS: i64 = 2;
pub const MAX_PRESENCE_HOURS: i64 = 48;
/// Heure (Paris) du rappel de l'objectif, le samedi.
pub const OBJECTIVE_HOUR: u32 = 18;
/// Un MP refusé n'est pas retenté avant ce délai.
pub const DM_RETRY_DAYS: i64 = 7;
/// Délai après la publication d'une présence avant le premier MP.
pub const PRESENCE_GRACE_MINUTES: i64 = 30;
pub const UNSUBSCRIBE: &str = "-# Pour ne plus recevoir ces rappels : `/rappels`";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Presence,
    Objective,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Presence => "presence",
            Self::Objective => "objective",
        }
    }
}

/// Réglages d'un serveur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub presence_hours: i64,
    pub objective: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { enabled: false, presence_hours: DEFAULT_PRESENCE_HOURS, objective: true }
    }
}

/// Choix d'un membre, et MP refusés.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemberPrefs {
    pub presence_off: bool,
    pub objective_off: bool,
    pub dm_closed_at: Option<DbTime>,
}

impl MemberPrefs {
    pub fn wants(&self, kind: Kind) -> bool {
        match kind {
            Kind::Presence => !self.presence_off,
            Kind::Objective => !self.objective_off,
        }
    }

    /// Faut-il tenter un MP ? Pas dans les 7 jours qui suivent un refus.
    pub fn reachable(&self, now: DateTime<Utc>) -> bool {
        self.dm_closed_at.is_none_or(|closed| now - closed.0 >= Duration::days(DM_RETRY_DAYS))
    }
}

/// La présence commence-t-elle dans moins de `hours` heures, et a-t-elle été
/// publiée depuis assez longtemps ?
pub fn presence_due(now: DateTime<Utc>, event: DbTime, created: DbTime, hours: i64) -> bool {
    event.0 > now
        && event.0 - now <= Duration::hours(hours)
        && now - created.0 >= Duration::minutes(PRESENCE_GRACE_MINUTES)
}

/// Samedi à partir de 18:00, heure de Paris.
pub fn objective_due(now: DateTime<Utc>) -> bool {
    let local = now.with_timezone(&TIMEZONE);
    local.weekday() == Weekday::Sat && local.hour() >= OBJECTIVE_HOUR
}

/// Destinataires : candidats hors exclus (absents, blocklist, bots), qui
/// veulent ce rappel et dont les MP ne sont pas fermés. Ordre conservé, sans doublon.
pub fn recipients(
    candidates: &[String],
    excluded: &HashSet<String>,
    prefs: &HashMap<String, MemberPrefs>,
    kind: Kind,
    now: DateTime<Utc>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    candidates
        .iter()
        .filter(|id| !excluded.contains(*id))
        .filter(|id| prefs.get(*id).is_none_or(|p| p.wants(kind) && p.reachable(now)))
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect()
}

fn server(guild_name: Option<&str>) -> String {
    guild_name.map(|name| format!(" sur **{name}**")).unwrap_or_default()
}

/// MP de présence : titre, heure (Paris), lien vers le message pour répondre.
pub fn presence_text(guild_name: Option<&str>, titre: &str, event: DbTime, link: Option<&str>) -> String {
    let hour = event.0.with_timezone(&TIMEZONE).format("%H:%M");
    let mut text = format!(
        "⏰ Présence **{titre}** à {hour} (<t:{}:R>){} : tu n'as pas répondu.",
        event.unix(),
        server(guild_name)
    );
    if let Some(link) = link {
        text.push_str(&format!("\nRéponds ici : {link}"));
    }
    text.push('\n');
    text.push_str(UNSUBSCRIBE);
    text
}

/// MP de l'objectif : où en est le membre et ce qu'il reste à verser.
pub fn objective_text(guild_name: Option<&str>, objective: &Objective, row: &ProgressRow) -> String {
    format!(
        "🎯 Objectif de cotisation de la semaine{} : {}.\nTu en es à **{}** : il te reste **{}** avant dimanche minuit.\n{UNSUBSCRIBE}",
        server(guild_name),
        objective.describe(),
        money(row.amount),
        money((row.target - row.amount).max(0))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cotisations::objective::Counts;
    use chrono::TimeZone;

    fn utc(d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, d, h, min, 0).unwrap()
    }

    #[test]
    fn presence_window() {
        let now = utc(20, 17, 0);
        let created = DbTime(utc(19, 12, 0));
        assert!(presence_due(now, DbTime(utc(20, 19, 0)), created, 2));
        assert!(!presence_due(now, DbTime(utc(20, 19, 1)), created, 2), "plus de 2 h avant");
        assert!(!presence_due(now, DbTime(utc(20, 16, 59)), created, 2), "déjà commencée");
        assert!(!presence_due(now, DbTime(utc(20, 18, 0)), DbTime(utc(20, 16, 45)), 2), "tout juste publiée");
        assert!(presence_due(now, DbTime(utc(21, 16, 0)), created, 24));
    }

    #[test]
    fn objective_on_saturday_evening_in_paris() {
        // Été (UTC+2) : samedi 24/10 16:00 UTC = 18:00 à Paris.
        assert!(objective_due(utc(24, 16, 0)));
        assert!(!objective_due(utc(24, 15, 59)));
        // Samedi 22:30 UTC = dimanche 00:30 à Paris : trop tard.
        assert!(!objective_due(utc(24, 22, 30)));
        // Hiver (UTC+1) : samedi 31/10 17:00 UTC = 18:00.
        assert!(objective_due(utc(31, 17, 0)));
        assert!(!objective_due(utc(30, 18, 0)), "vendredi");
    }

    #[test]
    fn selects_recipients() {
        let now = utc(20, 12, 0);
        let ids: Vec<String> = ["a", "b", "c", "d", "e", "a"].iter().map(|s| (*s).to_owned()).collect();
        let excluded: HashSet<String> = ["b".to_owned()].into();
        let prefs: HashMap<String, MemberPrefs> = [
            ("c".to_owned(), MemberPrefs { presence_off: true, ..Default::default() }),
            ("d".to_owned(), MemberPrefs { dm_closed_at: Some(DbTime(now - Duration::days(2))), ..Default::default() }),
            ("e".to_owned(), MemberPrefs { dm_closed_at: Some(DbTime(now - Duration::days(8))), ..Default::default() }),
        ]
        .into();
        assert_eq!(recipients(&ids, &excluded, &prefs, Kind::Presence, now), ["a", "e"]);
        // « c » n'a coupé que les présences.
        assert_eq!(recipients(&ids, &excluded, &prefs, Kind::Objective, now), ["a", "c", "e"]);
    }

    #[test]
    fn messages_explain_and_offer_unsubscribe() {
        let event = DbTime(utc(20, 19, 0));
        let text = presence_text(Some("Le Cartel"), "Réunion", event, Some("https://discord.com/channels/1/2/3"));
        assert!(text.starts_with("⏰ Présence **Réunion** à 21:00 (<t:"), "{text}");
        assert!(
            text.contains(
                " sur **Le Cartel** : tu n'as pas répondu.\nRéponds ici : https://discord.com/channels/1/2/3"
            )
        );
        assert!(text.ends_with(UNSUBSCRIBE));
        let objective = Objective { amount: 50_000, counts: Counts::Total, reminder: true };
        let row = ProgressRow { user_id: "1".into(), amount: 20_000, target: 50_000, reached: false, exempt: false };
        let text = objective_text(None, &objective, &row);
        assert!(text.contains("il te reste **30\u{202f}000\u{202f}$**"), "{text}");
        assert!(text.ends_with(UNSUBSCRIBE));
    }
}
