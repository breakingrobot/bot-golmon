//! Lecture des messages du webhook : en direct (MESSAGE_CREATE) et en
//! rattrapage (messages publiés pendant que le bot était arrêté).

use super::{parser, service};
use crate::{context::Ctx, util::time::DbTime};
use anyhow::Result;
use twilight_model::{
    channel::Message,
    id::{
        Id,
        marker::{ChannelMarker, MessageMarker},
    },
};

/// Au plus 50 pages de 100 messages par salon et par rattrapage.
const CATCH_UP_PAGES: usize = 50;

/// Enregistre les mouvements d'un message ; renvoie le nombre de nouveaux.
pub async fn record(ctx: &Ctx, guild_id: &str, message: &Message) -> Result<usize> {
    let mut added = 0;
    let fallback = DbTime(chrono::DateTime::from_timestamp_micros(message.timestamp.as_micros()).unwrap_or_default());
    for (index, embed) in message.embeds.iter().enumerate() {
        let Some(movement) = parser::parse(embed) else { continue };
        let id = format!("{}:{index}", message.id);
        let at = movement.at.unwrap_or(fallback);
        if service::insert(&ctx.db, &id, guild_id, &message.channel_id.to_string(), &movement, at).await? {
            added += 1;
        }
    }
    service::mark_read(&ctx.db, &message.channel_id.to_string(), message.id.get()).await?;
    Ok(added)
}

/// Message reçu en direct : seulement ceux des webhooks et bots, dans un salon suivi.
pub async fn on_message(ctx: &Ctx, message: &Message) {
    if message.embeds.is_empty() || (message.webhook_id.is_none() && !message.author.bot) {
        return;
    }
    let result = async {
        let Some(guild_id) = service::is_channel(&ctx.db, &message.channel_id.to_string()).await? else {
            return Ok(0);
        };
        record(ctx, &guild_id, message).await
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
pub async fn catch_up(
    ctx: &Ctx,
    guild_id: &str,
    channel_id: Id<ChannelMarker>,
    after: Option<Id<MessageMarker>>,
) -> Result<usize> {
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
                for message in &page {
                    added += record(ctx, guild_id, message).await?;
                }
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
                for message in &page {
                    added += record(ctx, guild_id, message).await?;
                }
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
    for (channel, last) in channels {
        let (Some(channel_id), last) = (
            channel.parse().ok().and_then(Id::new_checked),
            last.and_then(|l| l.parse().ok()).and_then(Id::new_checked),
        ) else {
            continue;
        };
        match catch_up(ctx, guild_id, channel_id, last).await {
            Ok(added) if added > 0 => tracing::info!(%channel_id, added, "coffres : messages manqués rattrapés"),
            Ok(_) => {}
            Err(error) => tracing::warn!(%channel_id, ?error, "coffres : rattrapage impossible"),
        }
    }
}
