//! Réponses aux interactions, avec suivi de l'état (répondu, différé) pour
//! que le gestionnaire d'erreur sache comment prévenir l'utilisateur.

use crate::{context::Context, discord::messages::Outgoing};
use anyhow::Result;
use std::sync::atomic::{AtomicU8, Ordering};
use twilight_model::{
    application::{command::CommandOptionChoice, interaction::Interaction},
    channel::message::{Component, Embed, MessageFlags},
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, RoleMarker, UserMarker},
    },
    user::User,
};

const PENDING: u8 = 0;
const ANSWERED: u8 = 1;

/// Message de réponse à une interaction.
#[derive(Default, Clone)]
pub struct Reply {
    pub message: Outgoing,
    pub ephemeral: bool,
}

impl Reply {
    pub fn text(content: impl Into<String>) -> Self {
        Self { message: Outgoing::text(content), ephemeral: false }
    }

    /// Réponse visible uniquement par l'auteur de l'interaction.
    pub fn ephemeral(content: impl Into<String>) -> Self {
        Self { message: Outgoing::text(content), ephemeral: true }
    }

    pub fn embed(embed: Embed) -> Self {
        Self { message: Outgoing::embeds(vec![embed]), ephemeral: false }
    }

    pub fn hidden(mut self) -> Self {
        self.ephemeral = true;
        self
    }

    pub fn with_embed(mut self, embed: Embed) -> Self {
        self.message.embeds.push(embed);
        self
    }

    /// Lignes de composants (boutons, menus) sous le message.
    pub fn with_components(mut self, components: Vec<Component>) -> Self {
        self.message.components.extend(components);
        self
    }

    fn data(&self) -> InteractionResponseData {
        InteractionResponseData {
            content: self.message.content.clone(),
            embeds: (!self.message.embeds.is_empty()).then(|| self.message.embeds.clone()),
            components: (!self.message.components.is_empty()).then(|| self.message.components.clone()),
            allowed_mentions: Some(self.message.mentions.to_allowed()),
            flags: self.ephemeral.then_some(MessageFlags::EPHEMERAL),
            ..Default::default()
        }
    }
}

/// Une interaction en cours de traitement.
pub struct Ix {
    pub ctx: Context,
    pub interaction: Interaction,
    state: AtomicU8,
}

impl Ix {
    pub fn new(ctx: Context, interaction: Interaction) -> Self {
        Self { ctx, interaction, state: AtomicU8::new(PENDING) }
    }

    pub fn answered(&self) -> bool {
        self.state.load(Ordering::Acquire) != PENDING
    }

    pub fn user(&self) -> Option<&User> {
        self.interaction.member.as_ref().and_then(|m| m.user.as_ref()).or(self.interaction.user.as_ref())
    }

    pub fn user_id(&self) -> Id<UserMarker> {
        self.interaction.author_id().expect("une interaction a toujours un auteur")
    }

    pub fn username(&self) -> String {
        self.user().map(|u| u.name.clone()).unwrap_or_default()
    }

    pub fn guild_id(&self) -> Option<Id<GuildMarker>> {
        self.interaction.guild_id
    }

    pub fn channel_id(&self) -> Option<Id<ChannelMarker>> {
        self.interaction.channel.as_ref().map(|c| c.id)
    }

    /// Permissions calculées par Discord pour le membre dans ce salon.
    pub fn has_permission(&self, permission: Permissions) -> bool {
        self.interaction
            .member
            .as_ref()
            .and_then(|m| m.permissions)
            .is_some_and(|p| p.contains(Permissions::ADMINISTRATOR) || p.contains(permission))
    }

    pub fn member_roles(&self) -> &[Id<RoleMarker>] {
        self.interaction.member.as_ref().map(|m| m.roles.as_slice()).unwrap_or_default()
    }

    async fn respond(&self, kind: InteractionResponseType, data: Option<InteractionResponseData>) -> Result<()> {
        let response = InteractionResponse { kind, data };
        self.ctx.interaction().create_response(self.interaction.id, &self.interaction.token, &response).await?;
        self.state.store(ANSWERED, Ordering::Release);
        Ok(())
    }

    /// Première réponse avec un message.
    pub async fn reply(&self, reply: Reply) -> Result<()> {
        self.respond(InteractionResponseType::ChannelMessageWithSource, Some(reply.data())).await
    }

    /// Accuse réception ; la réponse viendra via `edit`.
    pub async fn defer(&self, ephemeral: bool) -> Result<()> {
        let data =
            InteractionResponseData { flags: ephemeral.then_some(MessageFlags::EPHEMERAL), ..Default::default() };
        self.respond(InteractionResponseType::DeferredChannelMessageWithSource, Some(data)).await
    }

    /// Accuse réception d'un composant sans modifier le message.
    pub async fn defer_update(&self) -> Result<()> {
        self.respond(InteractionResponseType::DeferredUpdateMessage, None).await
    }

    /// Remplace la réponse différée (ou initiale). Les composants sont toujours
    /// envoyés : sans composant, les boutons d'une réponse précédente disparaissent.
    pub async fn edit(&self, reply: Reply) -> Result<()> {
        let allowed = reply.message.mentions.to_allowed();
        let client = self.ctx.interaction();
        client
            .update_response(&self.interaction.token)
            .allowed_mentions(Some(&allowed))
            .content(reply.message.content.as_deref())
            .embeds(Some(&reply.message.embeds))
            .components(Some(&reply.message.components))
            .await?;
        Ok(())
    }

    /// Répond à un composant en remplaçant le message qui le porte (boutons compris).
    pub async fn update(&self, reply: Reply) -> Result<()> {
        let data = InteractionResponseData {
            content: reply.message.content.clone(),
            embeds: Some(reply.message.embeds.clone()),
            components: Some(reply.message.components.clone()),
            allowed_mentions: Some(reply.message.mentions.to_allowed()),
            ..Default::default()
        };
        self.respond(InteractionResponseType::UpdateMessage, Some(data)).await
    }

    pub async fn edit_text(&self, content: impl Into<String>) -> Result<()> {
        self.edit(Reply::text(content)).await
    }

    /// Message supplémentaire après une première réponse.
    pub async fn followup(&self, reply: Reply) -> Result<()> {
        let allowed = reply.message.mentions.to_allowed();
        let client = self.ctx.interaction();
        let mut request = client.create_followup(&self.interaction.token).allowed_mentions(Some(&allowed));
        if let Some(content) = &reply.message.content {
            request = request.content(content);
        }
        if !reply.message.embeds.is_empty() {
            request = request.embeds(&reply.message.embeds);
        }
        if !reply.message.components.is_empty() {
            request = request.components(&reply.message.components);
        }
        if reply.ephemeral {
            request = request.flags(MessageFlags::EPHEMERAL);
        }
        request.await?;
        Ok(())
    }

    pub async fn modal(&self, custom_id: String, title: String, components: Vec<Component>) -> Result<()> {
        let data = InteractionResponseData {
            custom_id: Some(custom_id),
            title: Some(crate::util::format::truncate(&title, 45)),
            components: Some(components),
            ..Default::default()
        };
        self.respond(InteractionResponseType::Modal, Some(data)).await
    }

    pub async fn autocomplete(&self, choices: Vec<CommandOptionChoice>) -> Result<()> {
        let data = InteractionResponseData { choices: Some(choices), ..Default::default() };
        self.respond(InteractionResponseType::ApplicationCommandAutocompleteResult, Some(data)).await
    }

    /// Réponse éphémère en tenant compte de l'état : réponse initiale si rien
    /// n'a encore été envoyé, message de suivi sinon.
    pub async fn notify(&self, content: impl Into<String>) -> Result<()> {
        if self.answered() {
            self.followup(Reply::ephemeral(content)).await
        } else {
            self.reply(Reply::ephemeral(content)).await
        }
    }
}

/// Choix d'autocomplétion (libellé tronqué à 100 caractères).
pub fn choice(name: &str, value: impl Into<String>) -> CommandOptionChoice {
    use twilight_model::application::command::CommandOptionChoiceValue;
    CommandOptionChoice {
        name: crate::util::format::truncate(name, 100),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(value.into()),
    }
}

pub const GUILD_ONLY: &str = "Commande utilisable en serveur uniquement.";
pub const MANAGE_GUILD_ONLY: &str =
    "Seuls les membres avec la permission « Gérer le serveur » peuvent modifier la configuration.";
