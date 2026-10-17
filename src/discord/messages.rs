//! Accès REST tolérants : salon supprimé, bot exclu, permission retirée sont
//! des cas normaux qui ne doivent pas faire tomber la commande en cours.

use crate::context::Ctx;
use anyhow::Result;
use twilight_http::{api_error::ApiError, error::ErrorType};
use twilight_model::{
    channel::{
        Message,
        message::{AllowedMentions, Component, Embed, MentionType, MessageFlags},
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
    pub flags: Option<MessageFlags>,
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

    /// Message « Components V2 » : ni `content` ni embeds, tout passe par les composants.
    pub fn components_v2(components: Vec<Component>) -> Self {
        Self { components, flags: Some(MessageFlags::IS_COMPONENTS_V2), ..Default::default() }
    }

    pub fn is_v2(&self) -> bool {
        self.flags.is_some_and(|flags| flags.contains(MessageFlags::IS_COMPONENTS_V2))
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
    // Avant `components` : twilight choisit la validation V1 ou V2 d'après les flags.
    if let Some(flags) = message.flags {
        request = request.flags(flags);
    }
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
    // `content` et `embeds` sont toujours envoyés : vides, ils effacent ceux
    // d'un ancien message embed converti en V2.
    let mut request = ctx
        .http
        .update_message(channel_id, message_id)
        .allowed_mentions(Some(&allowed))
        .content(message.content.as_deref())
        .embeds(Some(&message.embeds))
        .components(Some(&message.components));
    if let Some(flags) = message.flags {
        request = request.flags(flags);
    }
    Ok(request.await?.model().await?)
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

/// Requête refusée par Discord (4xx), hors limitation de débit.
pub fn is_rejected(error: &twilight_http::Error) -> bool {
    matches!(error.kind(), ErrorType::Response { status, .. } if (400..500).contains(&status.get()) && status.get() != 429)
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
///
/// Un ancien message à embeds peut refuser la conversion en Components V2 :
/// il est alors remplacé. Le nouveau est publié avant la suppression de
/// l'ancien, pour qu'un contenu invalide (refusé aux deux étapes) ne fasse pas
/// disparaître le tableau de bord.
pub async fn upsert(
    ctx: &Ctx,
    channel_id: Id<ChannelMarker>,
    known: Option<Id<MessageMarker>>,
    message: &Outgoing,
) -> Result<Option<Id<MessageMarker>>> {
    let mut replaced = None;
    if let Some(message_id) = known {
        match edit(ctx, channel_id, message_id, message).await {
            Ok(_) => return Ok(None),
            Err(error) => match error.downcast_ref::<twilight_http::Error>() {
                Some(http) if is_not_found(http) => {}
                Some(http) if message.is_v2() && is_rejected(http) => {
                    tracing::info!(%channel_id, %message_id, %http, "conversion en Components V2 refusée, message republié");
                    replaced = Some(message_id);
                }
                _ => return Err(error),
            },
        }
    }
    let sent = send(ctx, channel_id, message).await?;
    if let Some(message_id) = replaced {
        delete_quietly(ctx, channel_id, message_id).await;
    }
    Ok(Some(sent.id))
}

pub fn parse_id<T>(raw: &str) -> Option<Id<T>> {
    raw.parse().ok()
}
