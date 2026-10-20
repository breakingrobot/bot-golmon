//! Suggestions d'autocomplétion : filtrage et tri purs, partagés par les modules.
//! Discord affiche au plus 25 choix ; la saisie libre reste toujours possible.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Choix au plus par réponse d'autocomplétion.
pub const MAX_CHOICES: usize = 25;

/// Texte comparé sans casse ni accents (« Écurie » = « ecurie »).
pub fn fold(text: &str) -> String {
    deunicode::deunicode(text).to_lowercase()
}

/// La saisie (vide : tout) apparaît-elle dans l'un des textes ?
pub fn matches(typed: &str, texts: &[&str]) -> bool {
    let typed = fold(typed.trim());
    typed.is_empty() || texts.iter().any(|text| fold(text).contains(&typed))
}

/// Choix (libellé, valeur) correspondant à la saisie, sans doublon de valeur
/// (casse et accents ignorés : la première occurrence gagne), 25 au plus.
pub fn filter(candidates: impl IntoIterator<Item = (String, String)>, typed: &str) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    candidates
        .into_iter()
        .filter(|(label, value)| matches(typed, &[label, value]))
        .filter(|(_, value)| seen.insert(fold(value.trim())))
        .take(MAX_CHOICES)
        .collect()
}

fn weekday(day: NaiveDate) -> &'static str {
    match day.weekday() {
        Weekday::Mon => "lundi",
        Weekday::Tue => "mardi",
        Weekday::Wed => "mercredi",
        Weekday::Thu => "jeudi",
        Weekday::Fri => "vendredi",
        Weekday::Sat => "samedi",
        Weekday::Sun => "dimanche",
    }
}

/// Sens des dates proposées : à venir (absence, présence) ou passées (cotisation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dates {
    Upcoming,
    Past,
}

/// Dates JJ/MM/AAAA proposées autour d'aujourd'hui (deux semaines), filtrées
/// par la saisie. Une date complète et valide saisie à la main est proposée en
/// premier, même hors de la plage.
pub fn dates(today: NaiveDate, typed: &str, direction: Dates) -> Vec<(String, String)> {
    let label = |day: NaiveDate| {
        let date = format!("{} {}", weekday(day), day.format("%d/%m/%Y"));
        match (day - today).num_days() {
            0 => format!("Aujourd'hui · {date}"),
            1 => format!("Demain · {date}"),
            -1 => format!("Hier · {date}"),
            _ => date,
        }
    };
    let step = if direction == Dates::Upcoming { 1 } else { -1 };
    let mut candidates: Vec<(String, String)> = Vec::new();
    if let Ok(day) = NaiveDate::parse_from_str(typed.trim(), "%d/%m/%Y") {
        candidates.push((label(day), day.format("%d/%m/%Y").to_string()));
    }
    candidates.extend((0..14).map(|offset| {
        let day = today + Duration::days(offset * step);
        (label(day), day.format("%d/%m/%Y").to_string())
    }));
    // Des chiffres : début de la date ; sinon un mot du libellé (« vendredi », « demain »).
    let typed = typed.trim();
    let numeric = typed.chars().all(|c| c.is_ascii_digit() || c == '/');
    let mut seen = std::collections::HashSet::new();
    candidates
        .into_iter()
        .filter(|(label, value)| if numeric { value.starts_with(typed) } else { matches(typed, &[label]) })
        .filter(|(_, value)| seen.insert(value.clone()))
        .take(MAX_CHOICES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(label: &str, value: &str) -> (String, String) {
        (label.to_owned(), value.to_owned())
    }

    #[test]
    fn filters_without_case_accents_or_duplicates() {
        let candidates = vec![pair("🟢 Écurie Benny's", "Écurie Benny's"), pair("ecurie benny's", "ecurie benny's")];
        assert_eq!(filter(candidates.clone(), "ECUR"), [pair("🟢 Écurie Benny's", "Écurie Benny's")]);
        assert!(filter(candidates, "ballas").is_empty());
        let many: Vec<_> = (0..40).map(|i| pair(&format!("Groupe {i}"), &format!("g{i}"))).collect();
        assert_eq!(filter(many, "").len(), MAX_CHOICES);
    }

    #[test]
    fn suggests_dates_around_today() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let upcoming = dates(today, "", Dates::Upcoming);
        assert_eq!(upcoming.len(), 14);
        assert_eq!(upcoming[0], pair("Aujourd'hui · mardi 20/10/2026", "20/10/2026"));
        assert_eq!(upcoming[1], pair("Demain · mercredi 21/10/2026", "21/10/2026"));
        let past = dates(today, "", Dates::Past);
        assert_eq!(past[1], pair("Hier · lundi 19/10/2026", "19/10/2026"));
        // Par le début de la date, ou par le jour de la semaine.
        assert_eq!(
            dates(today, "2", Dates::Upcoming).iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(),
            [
                "20/10/2026",
                "21/10/2026",
                "22/10/2026",
                "23/10/2026",
                "24/10/2026",
                "25/10/2026",
                "26/10/2026",
                "27/10/2026",
                "28/10/2026",
                "29/10/2026",
            ]
        );
        assert_eq!(dates(today, "vendredi", Dates::Upcoming).len(), 2);
        // Une date complète hors plage reste proposée.
        assert_eq!(dates(today, "24/12/2026", Dates::Upcoming), [pair("jeudi 24/12/2026", "24/12/2026")]);
    }
}
