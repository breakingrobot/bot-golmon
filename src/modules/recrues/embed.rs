use super::{
    COLOR, MOTIF_PREVIEW, NAMESPACE, SELECT_MAX_OPTIONS, VETO_COLOR, Vote,
    service::{Entry, Session, VoteRow},
};
use crate::{
    discord::{
        components::{row, select_option, string_select},
        embed::{EmbedBuilder, footer, now},
    },
    util::{
        format::{field, mention, pack_fields, truncate, truncate_field},
        ids::encode_custom_id,
        time::format_day_label,
    },
};
use std::collections::HashMap;
use twilight_model::channel::message::{Component, Embed};

const FIELD_BUDGET: usize = 5200;
const MAX_FIELDS: usize = 24;

/// `✅ 3 · ❌ 1 · ⬜ 0` — toujours les trois choix, même à zéro.
pub fn format_tally(votes: &[VoteRow]) -> String {
    Vote::ORDER
        .iter()
        .map(|vote| format!("{} {}", vote.emoji(), votes.iter().filter(|v| v.choix == vote.as_str()).count()))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn vote_line(vote: &VoteRow, limit: usize) -> String {
    let emoji = Vote::parse(&vote.choix).map_or("⬜", Vote::emoji);
    format!("{emoji} {} — {}", mention(&vote.voter_id), truncate(&vote.motif, limit))
}

fn veto_line(entry: &Entry) -> Option<String> {
    entry
        .veto_by_id
        .as_ref()
        .map(|by| format!("⛔ **Véto** de {} — {}", mention(by), entry.veto_motif.as_deref().unwrap_or("sans motif")))
}

/// Détail complet d'une recrue, pour les réponses éphémères.
pub fn format_entry_detail(entry: &Entry) -> String {
    let veto = veto_line(entry).map(|line| format!("{line}\n\n")).unwrap_or_default();
    let votes = if entry.votes.is_empty() {
        "*Aucun avis pour le moment.*".to_owned()
    } else {
        entry.votes.iter().map(|v| vote_line(v, 400)).collect::<Vec<_>>().join("\n")
    };
    truncate_field(&format!("{veto}{votes}"))
}

pub fn build_dashboard(session: &Session) -> Embed {
    let has_veto = session.entries.iter().any(|e| e.veto_by_id.is_some());
    let builder = EmbedBuilder::new()
        .title(format!("🗳️ Avis recrues — semaine du {}", format_day_label(session.week_start)))
        .color(if has_veto { VETO_COLOR } else { COLOR })
        .timestamp(now());

    if session.entries.is_empty() {
        return builder
            .description(
                "Aucune recrue cette semaine. Vérifie le rôle recrue avec `/recrue config`, ou ajoute quelqu'un via `/recrue ajouter`.",
            )
            .build();
    }

    // Recrues sous véto en tête : c'est l'information la plus décisive, et
    // `pack_fields` tronque par la fin si l'embed déborde.
    let mut entries: Vec<&Entry> = session.entries.iter().collect();
    entries.sort_by_key(|e| e.veto_by_id.is_none());

    let fields = entries
        .iter()
        .map(|entry| {
            let mut lines = vec![mention(&entry.user_id)];
            lines.extend(veto_line(entry));
            lines.extend(entry.votes.iter().map(|v| vote_line(v, MOTIF_PREVIEW)));
            if entry.votes.is_empty() {
                lines.push("*Aucun avis pour le moment.*".to_owned());
            }
            field(
                format!("{}{}", if entry.veto_by_id.is_some() { "⛔ " } else { "👤 " }, format_tally(&entry.votes)),
                truncate_field(&lines.join("\n")),
                false,
            )
        })
        .collect();

    let (kept, dropped) = pack_fields(fields, FIELD_BUDGET, MAX_FIELDS);
    let mut builder = builder;
    for f in kept {
        builder = builder.field(f);
    }
    if !dropped.is_empty() {
        builder = builder.field(field(
            "…",
            format!("+{} recrue(s) non affichée(s) — utilise `/recrue avis` pour les consulter.", dropped.len()),
            false,
        ));
    }
    let total_votes: usize = session.entries.iter().map(|e| e.votes.len()).sum();
    builder
        .footer(footer(format!(
            "{} recrue(s) · {total_votes} avis · un avis par gradé, modifiable",
            session.entries.len()
        )))
        .build()
}

/// Trois menus : voter, poser un véto, consulter le détail. Sans recrue, aucun
/// composant (un menu vide est refusé par l'API). Au-delà de 25 recrues, les
/// menus sont tronqués ; `/recrue avis` et `/recrue veto` restent utilisables.
pub fn build_components(session: &Session, labels: &HashMap<String, String>) -> Vec<Component> {
    let entries: Vec<&Entry> = session.entries.iter().take(SELECT_MAX_OPTIONS).collect();
    if entries.is_empty() {
        return Vec::new();
    }
    let select = |action: &str, placeholder: &str| {
        let options = entries
            .iter()
            .map(|entry| {
                let mut option = select_option(
                    truncate(labels.get(&entry.user_id).unwrap_or(&entry.user_id), 100),
                    entry.id.clone(),
                );
                if entry.veto_by_id.is_some() {
                    option.description = Some("⛔ Sous véto".to_owned());
                }
                option
            })
            .collect();
        row(vec![Component::SelectMenu(string_select(
            encode_custom_id(NAMESPACE, action, &session.id, None),
            placeholder,
            options,
        ))])
    };
    vec![
        select("vote", "🗳️ Donner mon avis sur…"),
        select("veto", "⛔ Poser un véto (leads)"),
        select("detail", "🔍 Voir les avis détaillés"),
    ]
}
