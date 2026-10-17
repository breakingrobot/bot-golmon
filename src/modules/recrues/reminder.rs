//! Rappel du dimanche 18:00 (Europe/Paris) : avant la clôture de la semaine à
//! minuit, les gradés qui n'ont pas donné leur avis sur toutes les recrues sont
//! mentionnés dans le salon des avis. Un seul rappel par session.

use super::service::{self, Entry};
use crate::{
    context::Ctx,
    discord::{
        members::role_member_ids,
        messages::{self, Mentions, Outgoing, parse_id},
    },
    util::time::TIMEZONE,
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use twilight_model::id::{Id, marker::GuildMarker};

/// Heure locale (Paris) à partir de laquelle le rappel part, le dimanche.
pub const REMINDER_HOUR: u32 = 18;
/// Discord refuse plus de 100 utilisateurs dans `allowed_mentions` : marge confortable.
const MENTIONS_PER_MESSAGE: usize = 50;
const MAX_MESSAGE_LENGTH: usize = 1900;
const HEADER: &str = "📋 Avis manquants avant la clôture de ce soir minuit : ";

/// Dimanche à partir de 18:00, heure de Paris. Le rappel est vérifié chaque
/// minute jusqu'à minuit : un redémarrage à 18:00 ne le fait pas manquer.
pub fn reminder_due(now: DateTime<Utc>) -> bool {
    let local = now.with_timezone(&TIMEZONE);
    local.weekday() == Weekday::Sun && local.hour() >= REMINDER_HOUR
}

/// Gradés qui n'ont pas donné leur avis sur toutes les recrues, avec le nombre
/// de recrues sans avis de leur part, dans l'ordre de `grade_ids`. Un gradé
/// lui-même suivi comme recrue n'a pas à donner son avis sur lui-même.
pub fn missing_votes(entries: &[Entry], grade_ids: &[String]) -> Vec<(String, usize)> {
    grade_ids
        .iter()
        .filter_map(|grade| {
            let missing = entries
                .iter()
                .filter(|entry| entry.user_id != *grade && !entry.votes.iter().any(|vote| vote.voter_id == *grade))
                .count();
            (missing > 0).then(|| (grade.clone(), missing))
        })
        .collect()
}

/// Textes du rappel : `<@a> (2 recrues), <@b> (1 recrue)`, découpés pour rester
/// sous les limites Discord (longueur et nombre de mentions autorisées).
pub fn reminder_messages(missing: &[(String, usize)]) -> Vec<(String, Vec<String>)> {
    let mut messages = Vec::new();
    let mut text = String::from(HEADER);
    let mut users: Vec<String> = Vec::new();
    for (user_id, count) in missing {
        let item = format!("<@{user_id}> ({count} recrue{})", if *count > 1 { "s" } else { "" });
        if !users.is_empty()
            && (users.len() == MENTIONS_PER_MESSAGE || text.len() + item.len() + 2 > MAX_MESSAGE_LENGTH)
        {
            messages.push((std::mem::replace(&mut text, String::from(HEADER)), std::mem::take(&mut users)));
        }
        if !users.is_empty() {
            text.push_str(", ");
        }
        text.push_str(&item);
        users.push(user_id.clone());
    }
    if !users.is_empty() {
        messages.push((text, users));
    }
    messages
}

/// Envoie le rappel de la session en cours, s'il n'est pas déjà parti.
pub async fn send_reminder(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(session) = service::current_session(&ctx.db, &guild).await? else {
        return Ok(());
    };
    if session.reminder_sent_at.is_some() || session.closed_at.is_some() || session.entries.is_empty() {
        return Ok(());
    }
    let Some(config) = service::get_config(&ctx.db, &guild).await? else {
        return Ok(());
    };
    let (Some(channel_id), Some(grade_role)) =
        (config.dashboard_channel_id.as_deref().and_then(parse_id), config.grade_role_id.as_deref().and_then(parse_id))
    else {
        return Ok(());
    };

    let grade_ids: Vec<String> = role_member_ids(ctx, guild_id, grade_role)
        .into_iter()
        .filter(|id| !parse_id(id).and_then(|id| ctx.cache.user(id)).is_some_and(|user| user.bot))
        .collect();
    // Cache des membres pas encore rempli (redémarrage) : on retentera à la minute suivante.
    if grade_ids.is_empty() {
        return Ok(());
    }

    let missing = missing_votes(&session.entries, &grade_ids);
    // La session est marquée même sans personne à relancer.
    if !service::claim_reminder(&ctx.db, &session.id).await? {
        return Ok(());
    }
    for (text, users) in reminder_messages(&missing) {
        let mentions = Mentions::Explicit {
            users: users.iter().filter_map(|id| parse_id(id)).collect(),
            roles: Vec::new(),
            everyone: false,
        };
        messages::send(ctx, channel_id, &Outgoing::text(text).with_mentions(mentions)).await?;
    }
    tracing::info!(%guild_id, missing = missing.len(), "rappel des avis recrues");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::recrues::service::VoteRow;
    use chrono::TimeZone;

    fn entry(id: &str, user_id: &str, voters: &[&str]) -> Entry {
        Entry {
            id: id.to_owned(),
            session_id: "s1".to_owned(),
            user_id: user_id.to_owned(),
            veto_by_id: None,
            veto_motif: None,
            votes: voters
                .iter()
                .map(|voter| VoteRow {
                    entry_id: id.to_owned(),
                    voter_id: (*voter).to_owned(),
                    choix: "POUR".to_owned(),
                    motif: "ok".to_owned(),
                })
                .collect(),
        }
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn lists_graders_missing_votes() {
        let entries = vec![entry("e1", "r1", &["a", "b"]), entry("e2", "r2", &["a"]), entry("e3", "r3", &[])];
        let missing = missing_votes(&entries, &ids(&["a", "b", "c"]));
        assert_eq!(missing, vec![("a".to_owned(), 1), ("b".to_owned(), 2), ("c".to_owned(), 3)]);
    }

    #[test]
    fn nobody_missing_when_everyone_voted() {
        let entries = vec![entry("e1", "r1", &["a", "b"]), entry("e2", "r2", &["b", "a"])];
        assert!(missing_votes(&entries, &ids(&["a", "b"])).is_empty());
        assert!(missing_votes(&[], &ids(&["a"])).is_empty());
        assert!(missing_votes(&entries, &[]).is_empty());
    }

    #[test]
    fn grader_does_not_vote_on_themselves() {
        // « r1 » est à la fois recrue et gradé : seul l'avis sur r2 lui manque.
        let entries = vec![entry("e1", "r1", &["a"]), entry("e2", "r2", &["a"])];
        assert_eq!(missing_votes(&entries, &ids(&["a", "r1"])), vec![("r1".to_owned(), 1)]);
    }

    #[test]
    fn formats_reminder_with_plurals() {
        let messages = reminder_messages(&[("1".to_owned(), 2), ("2".to_owned(), 1)]);
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].0,
            "📋 Avis manquants avant la clôture de ce soir minuit : <@1> (2 recrues), <@2> (1 recrue)"
        );
        assert_eq!(messages[0].1, ids(&["1", "2"]));
        assert!(reminder_messages(&[]).is_empty());
    }

    #[test]
    fn splits_long_reminders() {
        let missing: Vec<(String, usize)> =
            (0..120).map(|i| (format!("{}", 100_000_000_000_000_000u64 + i), 1)).collect();
        let messages = reminder_messages(&missing);
        assert!(messages.len() > 1);
        assert!(messages.iter().all(|(text, users)| text.len() <= MAX_MESSAGE_LENGTH && users.len() <= 50));
        assert_eq!(messages.iter().map(|(_, users)| users.len()).sum::<usize>(), 120);
    }

    #[test]
    fn due_on_paris_sunday_evening() {
        // Été (UTC+2) : dimanche 16:00 UTC = 18:00 à Paris.
        assert!(reminder_due(Utc.with_ymd_and_hms(2026, 8, 16, 16, 0, 0).unwrap()));
        assert!(!reminder_due(Utc.with_ymd_and_hms(2026, 8, 16, 15, 59, 0).unwrap()));
        // Hiver (UTC+1) : dimanche 17:00 UTC = 18:00 ; 23:30 UTC = lundi 00:30 à Paris.
        assert!(reminder_due(Utc.with_ymd_and_hms(2026, 1, 18, 17, 0, 0).unwrap()));
        assert!(!reminder_due(Utc.with_ymd_and_hms(2026, 1, 18, 23, 30, 0).unwrap()));
        // Samedi soir.
        assert!(!reminder_due(Utc.with_ymd_and_hms(2026, 1, 17, 19, 0, 0).unwrap()));
    }
}
