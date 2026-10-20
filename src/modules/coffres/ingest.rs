//! Lecture des messages du webhook : en direct (MESSAGE_CREATE) et en
//! rattrapage (messages publiés pendant que le bot était arrêté).
//!
//! Les nouveaux mouvements déclenchent les alertes (sauf au premier import de
//! l'historique d'un salon, qui inonderait le salon des alertes) et la mise à
//! jour du tableau du stock.

use super::{alerts, dashboard, parser, service};
use crate::{context::Context, util::time::DbTime};
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

/// Message publié par un webhook, et non une réponse à une commande
/// d'application : les applications « embed » (Carl-bot `/embed`…) répondent par
/// interaction et sont écartées. Créer un webhook dans le salon demande
/// « Gérer les webhooks » : réservé au staff.
fn from_webhook(message: &Message) -> bool {
    message.webhook_id.is_some() && message.interaction_metadata.is_none()
}

/// Mouvements lus dans un message (vide s'il ne vient pas d'un webhook).
fn movements_of(message: &Message) -> Vec<service::NewMovement> {
    if !from_webhook(message) {
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

/// Enregistre une page de messages d'un salon en une transaction ; renvoie les
/// nouveaux mouvements. Plusieurs webhooks du jeu peuvent publier dans le même
/// salon (une propriété chacun) : tout webhook qui publie un encadré « Coffre »
/// est reconnu, et chaque nouveau est signalé au journal du staff pour qu'une
/// source inattendue se voie (`/config coffre oublier_webhook` l'écarte).
async fn record(ctx: &Context, channel: &mut service::Channel, messages: &[Message]) -> Result<Vec<service::Recorded>> {
    let mut known: Vec<String> = service::webhooks(&ctx.db, &channel.guild_id, Some(&channel.channel_id))
        .await?
        .into_iter()
        .map(|w| w.webhook_id)
        .collect();
    let mut movements = Vec::new();
    for message in messages {
        let found = movements_of(message);
        let Some(webhook) = message.webhook_id.filter(|_| !found.is_empty()) else {
            continue;
        };
        let webhook = webhook.to_string();
        if !known.contains(&webhook) {
            let name = message.author.name.clone();
            if service::add_webhook(&ctx.db, &channel.guild_id, &channel.channel_id, &webhook, Some(&name)).await? {
                announce_webhook(ctx, channel, &webhook, &name).await;
            }
            channel.webhook_id.get_or_insert_with(|| webhook.clone());
            known.push(webhook);
        }
        movements.extend(found);
    }
    let last = messages.iter().map(|m| m.id.get()).max();
    service::insert_all(&ctx.db, &channel.guild_id, &channel.channel_id, &movements, last).await
}

/// Nouveau webhook reconnu : inscrit au journal du staff.
async fn announce_webhook(ctx: &Context, channel: &service::Channel, webhook: &str, name: &str) {
    tracing::info!(channel = %channel.channel_id, webhook, name, "coffres : nouveau webhook reconnu");
    let Some(guild_id) = channel.guild_id.parse().ok().and_then(Id::new_checked) else {
        return;
    };
    let details = format!(
        "**coffre** — webhook « {name} » (`{webhook}`) reconnu dans <#{}>. Si ce n'est pas le jeu : `/config coffre oublier_webhook:{webhook}`.",
        channel.channel_id
    );
    crate::modules::audit::record_web(
        ctx,
        guild_id,
        ctx.bot_user_id,
        &ctx.bot_username,
        crate::modules::audit::Entry::new(crate::modules::audit::Action::Config, None, details),
    )
    .await;
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
    if message.embeds.is_empty() || !from_webhook(message) {
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
            Err(error) if is_unknown_channel(&error) => forget_deleted_channel(ctx, &channel).await,
            Err(error) => tracing::warn!(channel_id = %channel.channel_id, ?error, "coffres : rattrapage impossible"),
        }
    }
}

/// Code d'erreur Discord « Unknown Channel » : le salon a été supprimé.
const UNKNOWN_CHANNEL: u64 = 10003;

/// L'erreur dit-elle que le salon n'existe plus ? (Pas un simple manque d'accès,
/// qui se répare en rendant le salon visible au bot.)
fn is_unknown_channel(error: &anyhow::Error) -> bool {
    use twilight_http::{api_error::ApiError, error::ErrorType};
    error.chain().filter_map(|cause| cause.downcast_ref::<twilight_http::Error>()).any(|http| {
        matches!(http.kind(), ErrorType::Response { error: ApiError::General(api), .. } if api.code == UNKNOWN_CHANNEL)
    })
}

/// Salon suivi supprimé de Discord : il n'est plus suivi (les mouvements déjà
/// lus restent), et le journal du staff le signale.
async fn forget_deleted_channel(ctx: &Context, channel: &service::Channel) {
    tracing::info!(channel_id = %channel.channel_id, "coffres : salon supprimé, il n'est plus suivi");
    if let Err(error) = service::remove_channel(&ctx.db, &channel.guild_id, &channel.channel_id).await {
        return tracing::error!(?error, "coffres : retrait du salon supprimé");
    }
    let Some(guild_id) = channel.guild_id.parse().ok().and_then(Id::new_checked) else {
        return;
    };
    let details = format!(
        "**coffre** — le salon `{}` a été supprimé de Discord : il n'est plus suivi (les mouvements déjà lus sont conservés).",
        channel.channel_id
    );
    crate::modules::audit::record_web(
        ctx,
        guild_id,
        ctx.bot_user_id,
        &ctx.bot_username,
        crate::modules::audit::Entry::new(crate::modules::audit::Action::Config, None, details),
    )
    .await;
}
