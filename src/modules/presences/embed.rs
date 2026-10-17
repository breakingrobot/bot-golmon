use super::{
    model::{COLOR_ACTIVE, COLOR_CLOSED, Presence, PresenceResponse, ResponseStatus},
    service::Roster,
};
use crate::{
    discord::{
        components::{ButtonExt as _, button, row},
        embed::{EmbedBuilder, EmbedBuilderExt as _, author, footer, now},
    },
    util::{
        format::{DESCRIPTION_LIMIT, field, format_mention_list, len16, truncate},
        ids::encode_custom_id,
    },
};
use std::collections::HashMap;
use twilight_model::channel::message::{Component, Embed, component::ButtonStyle, embed::EmbedField};

fn percent_bar(percent: i64) -> String {
    let filled = ((percent as f64 / 10.0).round() as usize).min(10);
    format!("{}{}", "🟩".repeat(filled), "⬜".repeat(10 - filled))
}

struct ChunkedField {
    field: EmbedField,
    /// Membres restants à partir de ce bloc, pour le résumé « … +N ».
    remaining: usize,
}

const CHUNK_SIZE: usize = 15;

fn chunked_fields(
    title: &str,
    user_ids: &[String],
    details: &HashMap<String, String>,
    empty: &str,
) -> Vec<ChunkedField> {
    if user_ids.is_empty() {
        return vec![ChunkedField { field: field(format!("{title} (0)"), empty, true), remaining: 0 }];
    }
    user_ids
        .chunks(CHUNK_SIZE)
        .enumerate()
        .map(|(index, chunk)| ChunkedField {
            field: field(
                if index == 0 { format!("{title} ({})", user_ids.len()) } else { "\u{200b}".to_owned() },
                format_mention_list(chunk, Some(details), "Aucun"),
                true,
            ),
            remaining: user_ids.len() - index * CHUNK_SIZE,
        })
        .collect()
}

pub fn build_embed(
    presence: &Presence,
    responses: &[PresenceResponse],
    roster: &Roster,
    guild_icon: Option<String>,
) -> Embed {
    let by_status = |status: ResponseStatus| -> Vec<&PresenceResponse> {
        responses.iter().filter(|r| r.status() == status).collect()
    };
    let (presents, absents, retards) =
        (by_status(ResponseStatus::Present), by_status(ResponseStatus::Absent), by_status(ResponseStatus::Retard));

    // 🔄 signale un changement d'avis ; les retards affichent leur durée.
    let details = |list: &[&PresenceResponse]| -> HashMap<String, String> {
        list.iter()
            .filter_map(|r| {
                let mut parts = Vec::new();
                if r.changed {
                    parts.push("🔄".to_owned());
                }
                if r.status() == ResponseStatus::Retard
                    && let Some(minutes) = r.retard_minutes
                {
                    parts.push(format!("{minutes} min"));
                }
                (!parts.is_empty()).then(|| (r.user_id.clone(), parts.join(" ")))
            })
            .collect()
    };
    let ids = |list: &[&PresenceResponse]| -> Vec<String> { list.iter().map(|r| r.user_id.clone()).collect() };

    let percent = if roster.effectif_total > 0 {
        ((presents.len() as f64 / roster.effectif_total as f64) * 100.0).round() as i64
    } else {
        0
    };

    let description = truncate(
        &format!(
            "Présence pour le <t:{}:F>{}\n\n{} {percent}%{}",
            presence.event_date.unix(),
            presence.description.as_deref().map(|d| format!("\n\n{d}")).unwrap_or_default(),
            percent_bar(percent),
            if presence.closed_at.is_some() { "\n\n🔒 **Présence clôturée**" } else { "" },
        ),
        DESCRIPTION_LIMIT,
    );

    let mut builder = EmbedBuilder::new()
        .author(author("Présence"))
        .title(truncate(&presence.titre, 256))
        .description(description.clone())
        .color(if presence.closed_at.is_some() { COLOR_CLOSED } else { COLOR_ACTIVE });

    let all = [
        chunked_fields("✅ Présents", &ids(&presents), &details(&presents), "Aucun"),
        chunked_fields("❌ Absents", &ids(&absents), &details(&absents), "Aucun"),
        chunked_fields("⏳ Retards", &ids(&retards), &details(&retards), "Aucun"),
        chunked_fields("🕓 Sans réponse", &roster.non_respondent_ids, &HashMap::new(), "Tout le monde a répondu"),
    ]
    .into_iter()
    .flatten();

    // Au-delà, l'embed dépasserait 25 champs ou 6000 caractères : le premier
    // bloc qui ne tient plus est résumé en « … +N autre(s) ».
    let mut count = 0;
    let mut length = len16(&presence.titre) + len16(&description) + 150;
    let mut capped = false;
    for chunk in all {
        if count >= 21 || length > 5000 {
            if !capped && chunk.remaining > 0 {
                builder = builder.field(field(chunk.field.name, format!("... +{} autre(s)", chunk.remaining), true));
                capped = true;
            }
            continue;
        }
        count += 1;
        length += len16(&chunk.field.name) + len16(&chunk.field.value);
        builder = builder.field(chunk.field);
    }

    builder
        .field(field("👥 Effectif", format!("{}/{}", responses.len(), roster.effectif_total), true))
        .field(field("📣 Émetteur", format!("<@{}>", presence.created_by_id), true))
        .field(field("📅 Date d'émission", format!("<t:{}:f>", presence.created_at.unix()), true))
        .footer(footer(&presence.id))
        .timestamp(now())
        .maybe_thumbnail(guild_icon)
        .build()
}

pub fn build_buttons(presence_id: &str, counts: [usize; 3], disabled: bool) -> Component {
    let id = |action: &str| encode_custom_id("presence", action, presence_id, None);
    row(vec![
        Component::Button(
            button(id("present"), format!("Présent ({})", counts[0]), ButtonStyle::Success)
                .with_emoji("✅")
                .disabled(disabled),
        ),
        Component::Button(
            button(id("absent"), format!("Absent ({})", counts[1]), ButtonStyle::Danger)
                .with_emoji("❌")
                .disabled(disabled),
        ),
        Component::Button(
            button(id("retard"), format!("Retard ({})", counts[2]), ButtonStyle::Secondary)
                .with_emoji("⏳")
                .disabled(disabled),
        ),
        Component::Button(button(id("ping"), "Ping", ButtonStyle::Primary).with_emoji("📌").disabled(disabled)),
    ])
}
