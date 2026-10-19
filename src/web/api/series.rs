//! Séries pour les graphiques : semaines (lundi de Paris) et jours, vides compris.
//!
//! Les bornes sont calculées ici, en heure de Paris (changements d'heure
//! compris), puis passées à SQLite dans une table `VALUES` : l'agrégation se
//! fait en une requête, par plages de dates indexées, et une période sans
//! donnée garde sa ligne (jointure externe).

use crate::util::time::{TIMEZONE, paris_day_start};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};

/// Jour de Paris d'un instant.
pub fn paris_date(time: DateTime<Utc>) -> NaiveDate {
    time.with_timezone(&TIMEZONE).date_naive()
}

/// Lundi de la semaine de `day`.
pub fn monday(day: NaiveDate) -> NaiveDate {
    day - Duration::days(i64::from(day.weekday().num_days_from_monday()))
}

/// Lundis des semaines qui couvrent `[first, last]`, dans l'ordre.
pub fn weeks(first: NaiveDate, last: NaiveDate) -> Vec<NaiveDate> {
    let mut weeks = Vec::new();
    let mut week = monday(first);
    while week <= last {
        weeks.push(week);
        week += Duration::weeks(1);
    }
    weeks
}

/// Les `count` dernières semaines jusqu'à celle de `today` comprise.
pub fn last_weeks(today: NaiveDate, count: i64) -> Vec<NaiveDate> {
    weeks(monday(today) - Duration::weeks(count - 1), today)
}

/// Jours de `[first, last]`.
pub fn days(first: NaiveDate, last: NaiveDate) -> Vec<NaiveDate> {
    first.iter_days().take_while(|day| *day <= last).collect()
}

/// Table `b("k", "s", "e")` : clé `YYYY-MM-DD`, début et fin (exclue) en UTC
/// au format de la base. `starts` est trié ; chaque période dure `step`.
pub fn buckets(starts: &[NaiveDate], step: Duration) -> (String, Vec<String>) {
    let rows = vec!["(?, ?, ?)"; starts.len()].join(", ");
    let sql = format!(r#"WITH b("k", "s", "e") AS (VALUES {rows})"#);
    let mut binds = Vec::with_capacity(starts.len() * 3);
    for start in starts {
        binds.push(start.format("%Y-%m-%d").to_string());
        binds.push(paris_day_start(*start).to_db_string());
        binds.push(paris_day_start(*start + step).to_db_string());
    }
    (sql, binds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn weeks_start_on_monday_and_include_gaps() {
        // Dimanche 25/10/2026 (changement d'heure) → lundi 02/11/2026.
        let list = weeks(day("2026-10-25"), day("2026-11-02"));
        assert_eq!(list, vec![day("2026-10-19"), day("2026-10-26"), day("2026-11-02")]);
        let last = last_weeks(day("2026-10-21"), 12);
        assert_eq!(last.len(), 12);
        assert_eq!(*last.last().unwrap(), day("2026-10-19"));
        assert_eq!(last[0], day("2026-08-03"));
        assert_eq!(days(day("2026-10-30"), day("2026-11-02")).len(), 4);
    }

    #[test]
    fn buckets_follow_paris_time() {
        let (sql, binds) = buckets(&[day("2026-10-19"), day("2026-10-26")], Duration::weeks(1));
        assert_eq!(sql, r#"WITH b("k", "s", "e") AS (VALUES (?, ?, ?), (?, ?, ?))"#);
        // Semaine du changement d'heure : commence en UTC+2, finit en UTC+1.
        assert_eq!(binds[..3], ["2026-10-19", "2026-10-18T22:00:00.000Z", "2026-10-25T23:00:00.000Z"]);
        assert_eq!(binds[4], "2026-10-25T23:00:00.000Z");
    }
}
