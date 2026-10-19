//! Lecture des messages du webhook : en direct (MESSAGE_CREATE) et en
//! rattrapage (messages publiés pendant que le bot était arrêté).
//!
//! Les nouveaux mouvements déclenchent les alertes (sauf au premier import de
//! l'historique d'un salon, qui inonderait le salon des alertes) et la mise à
//! jour du tableau du stock.

use super::{alerts, dashboard, parser, service};
use crate::{context::Context, util::time::DbTime};
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

/// Enregistre une page de messages d'un salon en une transaction ; renvoie les
/// nouveaux mouvements. Seul le webhook du jeu compte : le premier vu (le plus
/// fréquent de la page) est retenu, les messages des autres webhooks ou
/// applications sont ignorés, pour qu'un membre ne puisse pas imiter un encadré
/// « Coffre ».
async fn record(ctx: &Context, channel: &mut service::Channel, messages: &[Message]) -> Result<Vec<service::Recorded>> {
    if channel.webhook_id.is_none()
        && let Some(webhook) = likely_webhook(messages)
    {
        service::set_webhook(&ctx.db, &channel.channel_id, &webhook).await?;
        channel.webhook_id = Some(webhook);
    }
    let Some(webhook) = channel.webhook_id.as_deref() else {
        return Ok(Vec::new());
    };
    let movements: Vec<_> = messages
        .iter()
        .filter(|m| m.webhook_id.is_some_and(|id| id.to_string() == webhook))
        .flat_map(movements_of)
        .collect();
    let last = messages.iter().map(|m| m.id.get()).max();
    service::insert_all(&ctx.db, &channel.guild_id, &channel.channel_id, &movements, last).await
}

/// Suites d'un lot de nouveaux mouvements : alertes (si demandé) et tableau du stock.
async fn after_new(ctx: &Context, guild_id: &str, recorded: &[service::Recorded], alert: bool) {
    if recorded.is_empty() {
        return;
    }
    if alert && let Err(error) = alerts::notify(ctx, guild_id, recorded).await {
        tracing::warn!(guild = guild_id, ?error, "alerte des coffres impossible");
    }
    dashboard::request_refresh(ctx, guild_id);
}

/// Message reçu en direct : seulement ceux des webhooks, dans un salon suivi.
pub async fn on_message(ctx: &Context, message: &Message) {
    if message.embeds.is_empty() || message.webhook_id.is_none() {
        return;
    }
    let result = async {
        let Some(mut channel) = service::channel(&ctx.db, &message.channel_id.to_string()).await? else {
            return Ok(None);
        };
        let recorded = record(ctx, &mut channel, std::slice::from_ref(message)).await?;
        anyhow::Ok(Some((channel.guild_id, recorded)))
    }
    .await;
    match result {
        Ok(Some((guild_id, recorded))) if !recorded.is_empty() => {
            tracing::debug!(added = recorded.len(), "mouvements de coffre enregistrés");
            after_new(ctx, &guild_id, &recorded, true).await;
        }
        Ok(_) => {}
        Err(error) => tracing::error!(?error, "lecture d'un message de coffre"),
    }
}

/// Relit l'historique d'un salon : après le dernier message lu, ou les
/// `pages` dernières pages s'il n'a jamais été lu. Renvoie le nombre de
/// mouvements ajoutés. Le rattrapage vers le présent alerte (en un seul
/// message) ; le premier import, vers le passé, n'alerte pas.
pub async fn catch_up(ctx: &Context, channel: &mut service::Channel) -> Result<usize> {
    let Some(channel_id) = channel.channel_id.parse().ok().and_then(Id::<ChannelMarker>::new_checked) else {
        return Ok(0);
    };
    let after =
        channel.last_message_id.as_deref().and_then(|l| l.parse().ok()).and_then(Id::<MessageMarker>::new_checked);
    let mut added = Vec::new();
    let alert = after.is_some();
    let result = async {
        match after {
            // Vers le présent, page par page, à partir du dernier message lu.
            Some(mut cursor) => {
                for _ in 0..CATCH_UP_PAGES {
                    let mut page =
                        ctx.http.channel_messages(channel_id).after(cursor).limit(100).await?.model().await?;
                    if page.is_empty() {
                        break;
                    }
                    page.sort_by_key(|m| m.id);
                    added.extend(record(ctx, channel, &page).await?);
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
                    added.extend(record(ctx, channel, &page).await?);
                    match page.iter().map(|m| m.id).min() {
                        Some(oldest) if page.len() == 100 => before = Some(oldest),
                        _ => break,
                    }
                }
            }
        }
        anyhow::Ok(())
    }
    .await;
    // Ce qui a été enregistré avant une erreur est tout de même signalé.
    after_new(ctx, &channel.guild_id.clone(), &added, alert).await;
    result.map(|()| added.len())
}

/// Rattrapage de tous les salons suivis d'un serveur (à son arrivée sur la gateway).
pub async fn catch_up_guild(ctx: &Context, guild_id: &str) {
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
