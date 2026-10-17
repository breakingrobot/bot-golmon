//! Accès REST tolérants : salon supprimé, bot exclu, permission retirée sont
//! des cas normaux qui ne doivent pas faire tomber la commande en cours.

use crate::context::Ctx;
use anyhow::Result;
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_model::{
    channel::{
        Message,
        message::{AllowedMentions, Component, Embed, MentionType},
    },
    id::{
        Id,
        marker::{ChannelMarker, MessageMarker, RoleMarker, UserMarker},
    },
};

/// Contenu d'un message à publier ou à éditer.
#[derive(Default, Clone)]
pub struct Outgoing {
    pub content: Option<String>,
    pub embeds: Vec<Embed>,
    pub components: Vec<Component>,
    pub mentions: Mentions,
}

/// Mentions autorisées. Par défaut aucune : le bot ne pingue que lorsque c'est voulu.
#[derive(Default, Clone)]
pub enum Mentions {
    #[default]
    None,
    Users,
    Explicit {
        users: Vec<Id<UserMarker>>,
        roles: Vec<Id<RoleMarker>>,
        everyone: bool,
    },
}

impl Mentions {
    pub fn to_allowed(&self) -> AllowedMentions {
        match self {
            Self::None => AllowedMentions::default(),
            Self::Users => AllowedMentions { parse: vec![MentionType::Users], ..Default::default() },
            Self::Explicit { users, roles, everyone } => AllowedMentions {
                parse: if *everyone { vec![MentionType::Everyone] } else { Vec::new() },
                users: users.clone(),
                roles: roles.clone(),
                replied_user: false,
            },
        }
    }
}

impl Outgoing {
    pub fn text(content: impl Into<String>) -> Self {
        Self { content: Some(content.into()), ..Default::default() }
    }

    pub fn embeds(embeds: Vec<Embed>) -> Self {
        Self { embeds, ..Default::default() }
    }

    pub fn with_components(mut self, components: Vec<Component>) -> Self {
        self.components = components;
        self
    }

    pub fn with_mentions(mut self, mentions: Mentions) -> Self {
        self.mentions = mentions;
        self
    }
}

pub async fn send(ctx: &Ctx, channel_id: Id<ChannelMarker>, message: &Outgoing) -> Result<Message> {
    let allowed = message.mentions.to_allowed();
    let mut request = ctx.http.create_message(channel_id).allowed_mentions(Some(&allowed));
    if let Some(content) = &message.content {
        request = request.content(content);
    }
    if !message.embeds.is_empty() {
        request = request.embeds(&message.embeds);
    }
    if !message.components.is_empty() {
        request = request.components(&message.components);
    }
    Ok(request.await?.model().await?)
}

pub async fn edit(
    ctx: &Ctx,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    message: &Outgoing,
) -> Result<Message> {
    let allowed = message.mentions.to_allowed();
    Ok(ctx
        .http
        .update_message(channel_id, message_id)
        .allowed_mentions(Some(&allowed))
        .content(message.content.as_deref())
        .embeds(Some(&message.embeds))
        .components(Some(&message.components))
        .await?
        .model()
        .await?)
}

/// `true` si l'erreur signifie que la ressource n'existe plus (404, ou codes
/// Discord « Unknown Channel / Message »).
pub fn is_not_found(error: &twilight_http::Error) -> bool {
    match error.kind() {
        ErrorType::Response { status, error, .. } => {
            status.get() == 404 || matches!(error, ApiError::General(general) if matches!(general.code, 10003 | 10008))
        }
        _ => false,
    }
}

/// Récupère un message ; `Ok(None)` s'il a été supprimé (ou son salon).
pub async fn fetch(ctx: &Ctx, channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>) -> Result<Option<Message>> {
    match ctx.http.message(channel_id, message_id).await {
        Ok(response) => Ok(Some(response.model().await?)),
        Err(error) if is_not_found(&error) || is_forbidden(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn is_forbidden(error: &twilight_http::Error) -> bool {
    matches!(error.kind(), ErrorType::Response { status, .. } if status.get() == 403)
}

/// Supprime un message en ignorant les erreurs (déjà supprimé, droits retirés).
pub async fn delete_quietly(ctx: &Ctx, channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>) {
    if let Err(error) = ctx.http.delete_message(channel_id, message_id).await
        && !is_not_found(&error)
    {
        tracing::debug!(%channel_id, %message_id, %error, "suppression de message impossible");
    }
}

/// Édite le message mémorisé s'il existe encore, sinon en publie un nouveau.
/// Renvoie l'identifiant du nouveau message quand il a fallu republier.
pub async fn upsert(
    ctx: &Ctx,
    channel_id: Id<ChannelMarker>,
    known: Option<Id<MessageMarker>>,
    message: &Outgoing,
) -> Result<Option<Id<MessageMarker>>> {
    if let Some(message_id) = known {
        match edit(ctx, channel_id, message_id, message).await {
            Ok(_) => return Ok(None),
            Err(error) => {
                let gone = error.downcast_ref::<twilight_http::Error>().is_some_and(is_not_found);
                if !gone {
                    return Err(error);
                }
            }
        }
    }
    Ok(Some(send(ctx, channel_id, message).await?.id))
}

pub fn parse_id<T>(raw: &str) -> Option<Id<T>> {
    raw.parse().ok()
}
