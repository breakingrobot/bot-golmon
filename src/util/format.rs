//! Mise en forme bornée aux limites Discord.
//!
//! 1024 caractères par champ d'embed, 2000 par message, 25 champs et 6000
//! caractères cumulés sur l'ensemble des embeds d'un message. Une mention fait
//! ~22 caractères : un rôle d'une cinquantaine de membres suffit à dépasser la
//! limite d'un champ et à faire échouer l'édition (`Invalid Form Body`). Ces
//! helpers bornent systématiquement les listes.

use std::collections::HashMap;
use twilight_model::channel::message::embed::EmbedField;

pub const FIELD_LIMIT: usize = 1024;
pub const DESCRIPTION_LIMIT: usize = 4096;
pub const MESSAGE_LIMIT: usize = 2000;
const EMBED_TOTAL_LIMIT: usize = 6000;
const EMBED_MAX_FIELDS: usize = 25;

/// Les limites Discord se comptent en unités UTF-16, comme en JavaScript.
pub fn len16(text: &str) -> usize {
    text.encode_utf16().count()
}

pub fn mention(user_id: impl std::fmt::Display) -> String {
    format!("<@{user_id}>")
}

pub fn role_mention(role_id: impl std::fmt::Display) -> String {
    format!("<@&{role_id}>")
}

/// Un véhicule appartient soit à un membre, soit à un rôle (une organisation).
pub fn owner_mention(owner_id: &str, is_role: bool) -> String {
    if is_role { role_mention(owner_id) } else { mention(owner_id) }
}

/// Tronque un texte libre, en ajoutant « … » s'il dépasse `limit`.
pub fn truncate(text: &str, limit: usize) -> String {
    if len16(text) <= limit {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let width = ch.len_utf16();
        if used + width > limit - 1 {
            break;
        }
        used += width;
        out.push(ch);
    }
    out.push('…');
    out
}

pub fn truncate_field(text: &str) -> String {
    truncate(text, FIELD_LIMIT)
}

/// Liste de mentions pour un champ d'embed, tronquée à la limite de caractères.
pub fn format_mention_list(user_ids: &[String], details: Option<&HashMap<String, String>>, empty: &str) -> String {
    if user_ids.is_empty() {
        return empty.to_owned();
    }

    let mut text = String::new();
    let mut shown = 0;
    for id in user_ids {
        let line = match details.and_then(|map| map.get(id)) {
            Some(detail) => format!("{} ({detail})", mention(id)),
            None => mention(id),
        };
        if len16(&text) + len16(&line) + 1 > FIELD_LIMIT - 20 {
            continue;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&line);
        shown += 1;
    }

    let remaining = user_ids.len() - shown;
    if remaining > 0 { format!("{text}\n… +{remaining} autre{}", if remaining > 1 { "s" } else { "" }) } else { text }
}

/// Découpe une liste de mentions en messages sous la limite de 2000
/// caractères, chacun préfixé par `prefix`.
pub fn chunk_mentions(user_ids: &[String], prefix: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for id in user_ids {
        let candidate =
            if current.is_empty() { format!("{prefix} {}", mention(id)) } else { format!("{current} {}", mention(id)) };
        if len16(&candidate) > MESSAGE_LIMIT {
            chunks.push(std::mem::take(&mut current));
            current = format!("{prefix} {}", mention(id));
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub fn field(name: impl Into<String>, value: impl Into<String>, inline: bool) -> EmbedField {
    EmbedField { name: name.into(), value: value.into(), inline }
}

/// Coupe la liste de champs au premier qui ne tient plus dans le budget de
/// l'embed et renvoie le nombre de champs écartés. Sans ce garde-fou, un embed
/// trop gros fait échouer l'édition et le tableau de bord se fige.
///
/// Les champs gardés sont toujours un préfixe : les appelants trient par ordre
/// d'importance décroissante.
pub fn pack_fields(
    mut fields: Vec<EmbedField>,
    budget: usize,
    max_fields: usize,
) -> (Vec<EmbedField>, Vec<EmbedField>) {
    let max_fields = max_fields.min(EMBED_MAX_FIELDS);
    let budget = budget.min(EMBED_TOTAL_LIMIT);
    let mut used = 0;
    let mut count = 0;
    for field in &fields {
        let cost = len16(&field.name) + len16(&field.value);
        if count >= max_fields || used + cost > budget {
            break;
        }
        used += cost;
        count += 1;
    }
    let dropped = fields.split_off(count);
    (fields, dropped)
}

/// Format monétaire français : `1 500 000,5`.
pub fn format_price(value: f64) -> String {
    let negative = value < 0.0;
    let rounded = (value.abs() * 1000.0).round() / 1000.0;
    let integer = rounded.trunc() as u64;
    let mut decimals = format!("{:.3}", rounded.fract());
    decimals = decimals.trim_start_matches('0').trim_start_matches('.').trim_end_matches('0').to_owned();

    let digits = integer.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('\u{202f}');
        }
        grouped.push(ch);
    }
    let sign = if negative { "-" } else { "" };
    if decimals.is_empty() { format!("{sign}{grouped}") } else { format!("{sign}{grouped},{decimals}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_prices_like_intl_fr() {
        assert_eq!(format_price(1500.0), "1\u{202f}500");
        assert_eq!(format_price(150000.5), "150\u{202f}000,5");
        assert_eq!(format_price(12.0), "12");
        assert_eq!(format_price(1234567.891), "1\u{202f}234\u{202f}567,891");
    }

    #[test]
    fn chunks_mentions_under_limit() {
        let ids: Vec<String> = (0..300).map(|i| format!("{}", 100_000_000_000_000_000u64 + i)).collect();
        let chunks = chunk_mentions(&ids, "📣");
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| len16(c) <= MESSAGE_LIMIT && c.starts_with("📣")));
        let total: usize = chunks.iter().map(|c| c.matches("<@").count()).sum();
        assert_eq!(total, 300);
    }

    #[test]
    fn packs_fields_by_prefix() {
        let fields = (0..30).map(|i| field(format!("f{i}"), "x".repeat(300), false)).collect();
        let (kept, dropped) = pack_fields(fields, 5200, 24);
        assert_eq!(kept.len(), 17);
        assert_eq!(dropped.len(), 13);
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
    }
}
