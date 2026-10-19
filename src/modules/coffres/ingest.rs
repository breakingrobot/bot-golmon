//! Lecture des messages du webhook : en direct (MESSAGE_CREATE) et en
//! rattrapage (messages publiés pendant que le bot était arrêté).

use super::{parser, service};
use crate::{context::Ctx, util::time::DbTime};
use anyhow::Result;
use std::collections::HashMap;
use twilight_model::{
    channel::Message,
    id::{
        Id,
        marker::{ChannelMarker, MessageMarker},
    },
};

/// Au plus 50 pages de 100 messages par salon et par rattrapage.
const CATCH_UP_PAGES: usize = 50;

/// Mouvements lus dans un message (vide s'il ne vient pas d'un webhook).
fn movements_of(message: &Message) -> Vec<service::NewMovement> {
    if message.webhook_id.is_none() {
        return Vec::new();
    }
    let fallback = DbTime(chrono::DateTime::from_timestamp_micros(message.timestamp.as_micros()).unwrap_or_default());
    message
        .embeds
        .iter()
        .enumerate()
        .filter_map(|(index, embed)| {
            let movement = parser::parse(embed)?;
            let at = movement.at.unwrap_or(fallback);
            Some(service::NewMovement { id: format!("{}:{index}", message.id), movement, at })
        })
        .collect()
}

/// Webhook le plus fréquent parmi les messages qui portent des mouvements.
fn likely_webhook(messages: &[Message]) -> Option<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for message in messages.iter().filter(|m| !movements_of(m).is_empty()) {
        if let Some(id) = message.webhook_id {
            *counts.entry(id.to_string()).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(id, _)| id)
}

/// Enregistre une page de messages d'un salon en une transaction ; renvoie le
/// nombre de nouveaux mouvements. Seul le webhook du jeu compte : le premier vu
/// (le plus fréquent de la page) est retenu, les messages des autres webhooks
/// ou applications sont ignorés, pour qu'un membre ne puisse pas imiter un
/// encadré « Coffre ».
async fn record(ctx: &Ctx, channel: &mut service::Channel, messages: &[Message]) -> Result<usize> {
    if channel.webhook_id.is_none()
        && let Some(webhook) = likely_webhook(messages)
    {
        service::set_webhook(&ctx.db, &channel.channel_id, &webhook).await?;
        channel.webhook_id = Some(webhook);
    }
    let Some(webhook) = channel.webhook_id.as_deref() else {
        return Ok(0);
    };
    let movements: Vec<_> = messages
        .iter()
        .filter(|m| m.webhook_id.is_some_and(|id| id.to_string() == webhook))
        .flat_map(movements_of)
        .collect();
    let last = messages.iter().map(|m| m.id.get()).max();
    service::insert_all(&ctx.db, &channel.guild_id, &channel.channel_id, &movements, last).await
}

/// Message reçu en direct : seulement ceux des webhooks, dans un salon suivi.
pub async fn on_message(ctx: &Ctx, message: &Message) {
    if message.embeds.is_empty() || message.webhook_id.is_none() {
        return;
    }
    let result = async {
        let Some(mut channel) = service::channel(&ctx.db, &message.channel_id.to_string()).await? else {
            return Ok(0);
        };
        record(ctx, &mut channel, std::slice::from_ref(message)).await
    }
    .await;
    match result {
        Ok(added) if added > 0 => tracing::debug!(added, "mouvements de coffre enregistrés"),
        Ok(_) => {}
        Err(error) => tracing::error!(?error, "lecture d'un message de coffre"),
    }
}

/// Relit l'historique d'un salon : après le dernier message lu, ou les
/// `pages` dernières pages s'il n'a jamais été lu. Renvoie le nombre de mouvements ajoutés.
pub async fn catch_up(ctx: &Ctx, channel: &mut service::Channel) -> Result<usize> {
    let Some(channel_id) = channel.channel_id.parse().ok().and_then(Id::<ChannelMarker>::new_checked) else {
        return Ok(0);
    };
    let after =
        channel.last_message_id.as_deref().and_then(|l| l.parse().ok()).and_then(Id::<MessageMarker>::new_checked);
    let mut added = 0;
    match after {
        // Vers le présent, page par page, à partir du dernier message lu.
        Some(mut cursor) => {
            for _ in 0..CATCH_UP_PAGES {
                let mut page = ctx.http.channel_messages(channel_id).after(cursor).limit(100).await?.model().await?;
                if page.is_empty() {
                    break;
                }
                page.sort_by_key(|m| m.id);
                added += record(ctx, channel, &page).await?;
                cursor = page.last().expect("page non vide").id;
                if page.len() < 100 {
                    break;
                }
            }
        }
        // Première lecture : vers le passé, depuis le message le plus récent.
        None => {
            let mut before: Option<Id<MessageMarker>> = None;
            for _ in 0..CATCH_UP_PAGES {
                let request = ctx.http.channel_messages(channel_id);
                let page = match before {
                    Some(before) => request.before(before).limit(100).await?,
                    None => request.limit(100).await?,
                }
                .model()
                .await?;
                added += record(ctx, channel, &page).await?;
                match page.iter().map(|m| m.id).min() {
                    Some(oldest) if page.len() == 100 => before = Some(oldest),
                    _ => break,
                }
            }
        }
    }
    Ok(added)
}

/// Rattrapage de tous les salons suivis d'un serveur (à son arrivée sur la gateway).
pub async fn catch_up_guild(ctx: &Ctx, guild_id: &str) {
    let channels = match service::channels(&ctx.db, guild_id).await {
        Ok(channels) => channels,
        Err(error) => return tracing::error!(?error, "lecture des salons de coffre"),
    };
    for mut channel in channels {
        match catch_up(ctx, &mut channel).await {
            Ok(added) if added > 0 => {
                tracing::info!(channel_id = %channel.channel_id, added, "coffres : messages manqués rattrapés")
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(channel_id = %channel.channel_id, ?error, "coffres : rattrapage impossible"),
        }
    }
}
