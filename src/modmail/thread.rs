//! Opérations sur un ticket (`src/data/Thread.js` et `src/data/threads.js`).

use super::{
    Modmail,
    attachments::FileData,
    formatters,
    models::{Thread, ThreadMessage, message_type, thread_status},
    store::{self, NewMessage},
    util::{chunk_message_lines, escape_markdown, escape_name_formatting, humanize_duration, slugify},
};
use crate::{
    context::Ctx,
    discord::messages::{Mentions, is_not_found, parse_id},
};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;
use twilight_http::{api_error::ApiError, error::ErrorType, request::AuditLogReason as _};
use twilight_model::{
    channel::{
        ChannelType, Message,
        message::{Embed, MessageActivityType},
        thread::AutoArchiveDuration,
    },
    guild::Member,
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, MessageMarker, RoleMarker, UserMarker},
    },
    user::User,
};

/// Message à publier dans le salon d'un ticket.
#[derive(Default)]
pub struct Post {
    pub content: String,
    pub mentions: Mentions,
    pub reply_to: Option<Id<MessageMarker>>,
    pub files: Vec<FileData>,
    pub embeds: Vec<Embed>,
    pub components: Vec<twilight_model::channel::message::Component>,
}

impl Post {
    pub fn text(content: impl Into<String>) -> Self {
        Self { content: content.into(), ..Default::default() }
    }
}

/// Modérateur qui répond, tel que vu sur le serveur inbox.
pub struct Moderator {
    pub user: User,
    pub nick: Option<String>,
    pub roles: Vec<Id<RoleMarker>>,
}

impl Moderator {
    pub fn id(&self) -> Id<UserMarker> {
        self.user.id
    }
}

/// Contenu d'une réponse du staff.
pub struct StaffReply<'a> {
    pub text: &'a str,
    pub attachments: &'a [twilight_model::channel::Attachment],
    /// Réponse anonyme : seul le rôle est affiché à l'utilisateur.
    pub anonymous: bool,
    /// Message cité (réponse en ligne), côté salon du ticket.
    pub reference: Option<Id<MessageMarker>>,
}

/// Options de création d'un ticket (`CreateNewThreadForUserOpts`).
#[derive(Default)]
pub struct NewThreadOptions {
    /// Ne pas mentionner `mentionRole`.
    pub quiet: bool,
    /// Ignorer `requiredAccountAge` et `requiredTimeOnServer`.
    pub ignore_requirements: bool,
}

fn api_code(error: &twilight_http::Error) -> Option<u64> {
    match error.kind() {
        ErrorType::Response { error: ApiError::General(general), .. } => Some(general.code),
        _ => None,
    }
}

/// Date de création d'un compte, d'après son identifiant Discord.
pub fn snowflake_timestamp_ms(id: u64) -> i64 {
    ((id >> 22) + 1_420_070_400_000) as i64
}

impl Modmail {
    pub fn display_name(&self, user: &User) -> String {
        if self.config.use_displaynames {
            user.global_name.clone().unwrap_or_else(|| user.name.clone())
        } else {
            user.name.clone()
        }
    }

    /// Canal privé avec l'utilisateur (mis en cache : l'appel API est idempotent).
    pub async fn dm_channel(&self, ctx: &Ctx, user_id: Id<UserMarker>) -> Result<Id<ChannelMarker>> {
        if let Some(channel) = self.dm_channels.lock().expect("dm_channels").get(&user_id) {
            return Ok(*channel);
        }
        let channel = ctx.http.create_private_channel(user_id).await?.model().await?;
        self.dm_channels.lock().expect("dm_channels").insert(user_id, channel.id);
        Ok(channel.id)
    }

    async fn send_dm(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        content: &str,
        files: &[FileData],
        reply_to: Option<Id<MessageMarker>>,
    ) -> Result<Message> {
        let user_id = parse_id(&thread.user_id).ok_or_else(|| anyhow!("utilisateur invalide"))?;
        let channel_id = self.dm_channel(ctx, user_id).await.map_err(|_| {
            anyhow!(
                "Impossible d'ouvrir les MP avec l'utilisateur. Il a peut-être bloqué le bot ou restreint ses paramètres de confidentialité."
            )
        })?;
        let uploads = FileData::to_uploads(files);
        let mut request = ctx.http.create_message(channel_id).content(content);
        if !uploads.is_empty() {
            request = request.attachments(&uploads);
        }
        if let Some(reply_to) = reply_to {
            request = request.reply(reply_to).fail_if_not_exists(false);
        }
        Ok(request.await?.model().await?)
    }

    /// Publie dans le salon du ticket, en plusieurs messages si nécessaire.
    /// Renvoie le premier message ; `None` si le salon n'existe plus (le ticket
    /// est alors fermé automatiquement).
    pub async fn post_to_thread(&self, ctx: &Ctx, thread: &Thread, post: Post) -> Result<Option<Message>> {
        let Some(channel_id) = thread.channel_id.as_deref().and_then(parse_id::<ChannelMarker>) else {
            return Ok(None);
        };
        let chunks = if post.content.is_empty() { vec![String::new()] } else { chunk_message_lines(&post.content) };
        let allowed = post.mentions.to_allowed();
        let uploads = FileData::to_uploads(&post.files);
        let mut first: Option<Message> = None;

        for (index, chunk) in chunks.iter().enumerate() {
            let last = index == chunks.len() - 1;
            let mut request = ctx.http.create_message(channel_id).allowed_mentions(Some(&allowed));
            if !chunk.is_empty() {
                request = request.content(chunk);
            }
            if last {
                if !uploads.is_empty() {
                    request = request.attachments(&uploads);
                }
                if !post.embeds.is_empty() {
                    request = request.embeds(&post.embeds);
                }
                if !post.components.is_empty() {
                    request = request.components(&post.components);
                }
            }
            if let (0, Some(reply_to)) = (index, post.reply_to) {
                request = request.reply(reply_to).fail_if_not_exists(false);
            }
            match request.await {
                Ok(response) => {
                    let message = response.model().await?;
                    first.get_or_insert(message);
                }
                Err(error)
                    if api_code(&error) == Some(10003) || (is_not_found(&error) && api_code(&error) != Some(10008)) =>
                {
                    tracing::info!(user = %thread.user_name, "salon du ticket introuvable, fermeture automatique");
                    Box::pin(self.close_thread(ctx, thread, true, false)).await?;
                    return Ok(None);
                }
                Err(error) if api_code(&error) == Some(240000) => {
                    tracing::info!(user = %thread.user_name, "message bloqué par le filtre de liens dangereux");
                    ctx.http
                        .create_message(channel_id)
                        .content("Impossible d'envoyer le message dans le salon du ticket : il contient un lien bloqué par le filtre de liens dangereux")
                        .await?;
                    return Ok(first);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(first)
    }

    /// Message du bot dans le salon du ticket (enregistré dans les logs).
    pub async fn post_system_message(&self, ctx: &Ctx, thread: &Thread, text: &str) -> Result<Option<Message>> {
        self.post_system_message_with(ctx, thread, text, Mentions::None, None).await
    }

    pub async fn post_system_message_with(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        text: &str,
        mentions: Mentions,
        reply_to: Option<Id<MessageMarker>>,
    ) -> Result<Option<Message>> {
        self.post_system_message_full(ctx, thread, text, Post { mentions, reply_to, ..Default::default() }).await
    }

    /// Message système avec options (mentions, réponse, boutons) ; le contenu est `text`.
    pub async fn post_system_message_full(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        text: &str,
        post: Post,
    ) -> Result<Option<Message>> {
        let content =
            formatters::system_thread_message(&ThreadMessage { body: Some(text.to_owned()), ..Default::default() });
        let message = self.post_to_thread(ctx, thread, Post { content, ..post }).await?;
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::SYSTEM,
                body: text.to_owned(),
                inbox_message_id: message.as_ref().map(|m| m.id.to_string()),
                dm_channel_id: self.cached_dm_channel(thread),
                ..Default::default()
            },
        )
        .await?;
        Ok(message)
    }

    fn cached_dm_channel(&self, thread: &Thread) -> Option<String> {
        let user_id = parse_id(&thread.user_id)?;
        self.dm_channels.lock().expect("dm_channels").get(&user_id).map(ToString::to_string)
    }

    pub async fn add_system_message_to_logs(&self, thread: &Thread, text: &str) -> Result<()> {
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage { message_type: message_type::SYSTEM, body: text.to_owned(), ..Default::default() },
        )
        .await?;
        Ok(())
    }

    /// Message du bot envoyé à l'utilisateur (réponse automatique, message de fermeture).
    pub async fn send_system_message_to_user(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        text: &str,
        post_to_thread: bool,
    ) -> Result<()> {
        let dm = self.send_dm(ctx, thread, text, &[], None).await?;
        let inbox = if post_to_thread {
            self.post_to_thread(
                ctx,
                thread,
                Post::text(formatters::system_to_user_thread_message(&ctx.bot_username, text)),
            )
            .await?
        } else {
            None
        };
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::SYSTEM_TO_USER,
                body: text.to_owned(),
                dm_channel_id: Some(dm.channel_id.to_string()),
                dm_message_id: Some(dm.id.to_string()),
                inbox_message_id: inbox.map(|m| m.id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Message hors logs (mention du staff à l'ouverture, etc.).
    pub async fn post_non_log_message(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        content: &str,
        mentions: Mentions,
    ) -> Result<Option<Message>> {
        self.post_to_thread(ctx, thread, Post { content: content.to_owned(), mentions, ..Default::default() }).await
    }

    pub async fn save_chat_message(&self, thread: &Thread, message: &Message, kind: i64) -> Result<()> {
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: kind,
                user_id: Some(message.author.id.to_string()),
                user_name: self.display_name(&message.author),
                body: message.content.clone(),
                dm_message_id: Some(message.id.to_string()),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Rôle affiché devant le nom du modérateur : choix pour ce ticket, sinon
    /// choix par défaut, sinon son rôle affiché séparément le plus haut.
    pub async fn moderator_display_role(
        &self,
        ctx: &Ctx,
        moderator: &Moderator,
        thread_id: Option<&str>,
    ) -> Result<Option<String>> {
        let moderator_id = moderator.id().to_string();
        let mut candidates = Vec::new();
        if let Some(thread) = thread_id {
            candidates.push(store::role_override(&self.db, &moderator_id, Some(thread)).await?);
        }
        candidates.push(store::role_override(&self.db, &moderator_id, None).await?);
        for role_id in candidates.into_iter().flatten() {
            if let Some(role) = parse_id::<RoleMarker>(&role_id).filter(|id| moderator.roles.contains(id))
                && let Some(cached) = ctx.cache.role(role)
            {
                return Ok(Some(cached.resource().name.clone()));
            }
        }
        let main_role = moderator
            .roles
            .iter()
            .filter_map(|id| ctx.cache.role(*id).map(|r| r.resource().clone()))
            .filter(|role| role.hoist)
            .max_by_key(|role| role.position);
        Ok(main_role.map(|role| role.name).or_else(|| self.config.fallback_role_name.clone()))
    }

    fn moderator_name(&self, moderator: &Moderator) -> String {
        let regular = self.display_name(&moderator.user);
        let name = match (&moderator.nick, self.config.use_nicknames) {
            (Some(nick), true) => nick.clone(),
            _ => regular,
        };
        if self.config.break_formatting_for_names { escape_name_formatting(&name) } else { name }
    }

    /// Remplace `{{snippet}}` par le texte du snippet. `Err(liste)` si des snippets
    /// inconnus sont utilisés et que `errorOnUnknownInlineSnippet` est actif.
    async fn render_inline_snippets(&self, text: &str) -> Result<std::result::Result<String, Vec<String>>> {
        if !(self.config.allow_snippets && self.config.allow_inline_snippets) {
            return Ok(Ok(text.to_owned()));
        }
        let snippets: HashMap<String, String> =
            store::all_snippets(&self.db).await?.into_iter().map(|s| (s.trigger.to_lowercase(), s.body)).collect();
        let pattern = format!(
            "(?i){}(\\s*\\S+?\\s*){}",
            regex::escape(&self.config.inline_snippet_start),
            regex::escape(&self.config.inline_snippet_end)
        );
        let regex = regex::Regex::new(&pattern)?;
        let mut unknown: Vec<String> = Vec::new();
        let rendered = regex
            .replace_all(text, |caps: &regex::Captures<'_>| {
                let trigger = caps[1].trim();
                match snippets.get(&trigger.to_lowercase()) {
                    Some(body) => body.clone(),
                    None => {
                        if !unknown.iter().any(|u| u == trigger) {
                            unknown.push(trigger.to_owned());
                        }
                        caps[0].to_owned()
                    }
                }
            })
            .into_owned();
        if self.config.error_on_unknown_inline_snippet && !unknown.is_empty() {
            return Ok(Err(unknown));
        }
        Ok(Ok(rendered))
    }

    /// Réponse d'un modérateur à l'utilisateur. Renvoie `false` si la réponse n'a
    /// pas pu être envoyée (le message de commande est alors conservé).
    pub async fn reply_to_user(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        moderator: &Moderator,
        reply: StaffReply<'_>,
    ) -> Result<bool> {
        let StaffReply { text, attachments, anonymous, reference } = reply;
        let moderator_name = self.moderator_name(moderator);
        let role_name = self.moderator_display_role(ctx, moderator, Some(&thread.id)).await?;

        // Réponse en citation : on retrouve le message correspondant côté DM.
        let mut dm_reference = None;
        if self.config.relay_inline_replies
            && let Some(reference) = reference
            && let Some(replied) = store::message_for_discord_id(&self.db, &thread.id, &reference.to_string()).await?
        {
            dm_reference = replied.dm_message_id.as_deref().and_then(parse_id);
        }

        let text = match self.render_inline_snippets(text).await? {
            Ok(text) => text,
            Err(unknown) => {
                self.post_system_message(
                    ctx,
                    thread,
                    &format!("Les snippets suivants utilisés dans la réponse n'existent pas :\n{}", unknown.join(", ")),
                )
                .await?;
                return Ok(false);
            }
        };

        let mut files = Vec::new();
        let mut links = Vec::new();
        for attachment in attachments {
            files.push(self.attachment_to_file(attachment).await?);
            links.push(self.save_attachment(ctx, attachment).await);
        }

        let mut message = store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::TO_USER,
                user_id: Some(moderator.id().to_string()),
                user_name: moderator_name,
                body: text.clone(),
                is_anonymous: anonymous,
                role_name,
                attachments: links,
                dm_channel_id: self.cached_dm_channel(thread),
                ..Default::default()
            },
        )
        .await?;

        let dm_content = formatters::staff_reply_dm(&self.config, &message);
        let inbox_content = formatters::staff_reply_thread_message(&self.config, &message);

        // Les réponses doivent rester éditables : elles tiennent en un seul message.
        if dm_content.encode_utf16().count() > 2000 || inbox_content.encode_utf16().count() > 2000 {
            store::delete_message(&self.db, message.id).await?;
            self.post_system_message(
                ctx,
                thread,
                "Réponse trop longue ! Elle doit faire moins de 2000 caractères au total, nom du modérateur compris.",
            )
            .await?;
            return Ok(false);
        }

        let dm = match self.send_dm(ctx, thread, &dm_content, &files, dm_reference).await {
            Ok(dm) => dm,
            Err(error) => {
                store::delete_message(&self.db, message.id).await?;
                self.post_system_message(ctx, thread, &format!("Erreur lors de la réponse à l'utilisateur : {error}"))
                    .await?;
                return Ok(false);
            }
        };

        if self.config.attachment_storage == super::config::AttachmentStorage::Original {
            let urls: Vec<String> = dm.attachments.iter().map(|a| a.url.clone()).collect();
            store::set_message_attachments(&self.db, message.id, &urls).await?;
        }
        store::set_message_ids(&self.db, message.id, Some(&dm.id.to_string()), None).await?;
        sqlx::query("UPDATE `thread_messages` SET `dm_channel_id` = ? WHERE `id` = ?")
            .bind(dm.channel_id.to_string())
            .bind(message.id)
            .execute(&self.db)
            .await?;
        message.dm_message_id = Some(dm.id.to_string());

        let inbox = self
            .post_to_thread(
                ctx,
                thread,
                Post { content: inbox_content, reply_to: reference, files, ..Default::default() },
            )
            .await?;
        if let Some(inbox) = inbox {
            store::set_message_ids(&self.db, message.id, None, Some(&inbox.id.to_string())).await?;
        }

        // Une nouvelle réponse interrompt une fermeture programmée.
        if thread.scheduled_close_at.is_some() {
            store::cancel_scheduled_close(&self.db, &thread.id).await?;
            self.post_system_message(
                ctx,
                thread,
                "Fermeture programmée de ce ticket annulée suite à une nouvelle réponse",
            )
            .await?;
        }

        if self.config.auto_alert {
            self.start_auto_alert(ctx, thread, moderator.id());
        }
        Ok(true)
    }

    /// Ajoute le modérateur aux alertes du ticket après `autoAlertDelay`.
    fn start_auto_alert(&self, ctx: &Ctx, thread: &Thread, moderator_id: Id<UserMarker>) {
        let Some(delay) = super::util::convert_delay_string_to_ms(&self.config.auto_alert_delay) else {
            return;
        };
        let db = self.db.clone();
        let thread_id = thread.id.clone();
        let _ = ctx;
        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay.max(0) as u64)).await;
            if let Ok(Some(current)) = store::find_by_id(&db, &thread_id).await
                && current.is_open()
            {
                let _ = store::add_alert(&db, &thread_id, &moderator_id.to_string()).await;
            }
        });
        if let Some(previous) =
            self.auto_alerts.lock().expect("auto_alerts").insert(thread.id.clone(), handle.abort_handle())
        {
            previous.abort();
        }
    }

    /// Message privé de l'utilisateur, relayé dans le salon du ticket.
    pub async fn receive_user_reply(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        message: &Message,
        skip_alert: bool,
    ) -> Result<()> {
        // Évite les doublons (récupération après coupure qui chevauche l'historique).
        if store::message_by_dm_id(&self.db, &message.id.to_string()).await?.is_some() {
            return Ok(());
        }

        let mut content = message.content.clone();
        let mut forwarded_embeds: Option<Vec<Embed>> = None;

        let poll_lines = |poll: &twilight_model::poll::Poll| -> Vec<String> {
            let title = poll
                .question
                .text
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or("*(sondage sans titre)*")
                .to_owned();
            let mut lines = vec!["*L'utilisateur a envoyé un sondage :*".to_owned(), format!("Titre : {title}")];
            for (index, answer) in poll.answers.iter().enumerate() {
                let text = answer
                    .poll_media
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .unwrap_or("*(option sans texte)*");
                lines.push(format!("Option {} : {text}", index + 1));
            }
            lines
        };

        // Message transféré : on reproduit son contenu, ses pièces jointes et un lien.
        if let Some(snapshot) = message.message_snapshots.first() {
            let forwarded = &snapshot.message;
            let is_embed_forward = !forwarded.embeds.is_empty();
            let mut lines = vec![
                if is_embed_forward { "*L'utilisateur a transféré un embed :*" } else { "*Message transféré :*" }
                    .to_owned(),
            ];
            let text = forwarded.content.trim();
            if !text.is_empty() {
                lines.push(text.to_owned());
            } else if !is_embed_forward {
                lines.push("*(pas de texte)*".to_owned());
            }
            if !forwarded.attachments.is_empty() {
                lines.push(String::new());
                lines.push("*Pièces jointes :*".to_owned());
                lines.extend(forwarded.attachments.iter().map(|a| a.url.clone()));
            }
            if let Some(reference) = &message.reference
                && let (Some(guild), Some(channel), Some(id)) =
                    (snapshot.guild_id.or(reference.guild_id), reference.channel_id, reference.message_id)
            {
                let link = format!("*Lien du message :* <https://discord.com/channels/{guild}/{channel}/{id}>");
                if lines.len() > 1 {
                    lines.push(String::new());
                }
                lines.push(link);
            }
            if is_embed_forward {
                forwarded_embeds = Some(forwarded.embeds.clone());
            }
            let block = lines.join("\n").trim().to_owned();
            content = [content, block].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n");
        }

        if let Some(poll) = &message.poll {
            content = [content, poll_lines(poll).join("\n")]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
        }

        let mut links = Vec::new();
        let mut small_links = Vec::new();
        let mut files = Vec::new();
        for attachment in &message.attachments {
            let saved = self.save_attachment(ctx, attachment).await;
            // Petites pièces jointes relayées en fichiers, les autres en liens.
            if self.config.relay_small_attachments_as_attachments
                && attachment.size <= self.config.small_attachment_limit
                && let Ok(file) = self.attachment_to_file(attachment).await
            {
                files.push(file);
                small_links.push(saved.clone());
            }
            links.push(saved);
        }

        let mut reply_to = None;
        if self.config.relay_inline_replies
            && let Some(referenced) = &message.referenced_message
            && let Some(replied) =
                store::message_for_discord_id(&self.db, &thread.id, &referenced.id.to_string()).await?
        {
            reply_to = replied.inbox_message_id.as_deref().and_then(parse_id);
        }

        // Invitations à une activité (écoute Spotify, partie…).
        if let Some(activity) = &message.activity {
            let party_id = activity.party_id.clone().unwrap_or_default();
            let application = message
                .application
                .as_ref()
                .map(|a| a.name.clone())
                .filter(|name| !name.is_empty())
                .or_else(|| party_id.starts_with("spotify:").then(|| "Spotify".to_owned()))
                .unwrap_or_else(|| "Application inconnue".to_owned());
            let action = match activity.kind {
                MessageActivityType::Join | MessageActivityType::JoinRequest => "rejoindre une partie",
                MessageActivityType::Spectate => "regarder",
                MessageActivityType::Listen => "écouter ensemble",
                _ => "faire quelque chose",
            };
            content = format!("{content}\n\n*<Ce message contient une invitation à {action} sur {application}>*")
                .trim()
                .to_owned();
        }

        if !message.sticker_items.is_empty() {
            let lines: Vec<String> = message
                .sticker_items
                .iter()
                .map(|s| {
                    format!(
                        "*Sticker envoyé « {} » :* https://media.discordapp.net/stickers/{}.webp?size=160",
                        s.name, s.id
                    )
                })
                .collect();
            content = format!("{content}\n\n{}", lines.join("\n"));
        }
        let content = content.trim().to_owned();

        let embeds_metadata = |embeds: &[Embed]| -> Value {
            if embeds.is_empty() {
                Value::Null
            } else {
                serde_json::to_value(embeds.iter().take(10).collect::<Vec<_>>()).unwrap_or(Value::Null)
            }
        };
        let stored = store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::FROM_USER,
                user_id: Some(thread.user_id.clone()),
                user_name: self.display_name(&message.author),
                body: content,
                dm_message_id: Some(message.id.to_string()),
                dm_channel_id: Some(message.channel_id.to_string()),
                attachments: links,
                small_attachments: small_links,
                metadata: Some(json!({
                    "embeds": embeds_metadata(&message.embeds),
                    "forwardedEmbeds": forwarded_embeds.as_deref().map_or(Value::Null, embeds_metadata),
                })),
                ..Default::default()
            },
        )
        .await?;

        let post = Post {
            content: formatters::user_reply_thread_message(&self.config, &stored),
            reply_to,
            files,
            embeds: forwarded_embeds.unwrap_or_default(),
            ..Default::default()
        };
        if let Some(inbox) = self.post_to_thread(ctx, thread, post).await? {
            store::set_message_ids(&self.db, stored.id, None, Some(&inbox.id.to_string())).await?;
        }

        if self.config.react_on_seen {
            let emoji = twilight_http::request::channel::reaction::RequestReactionType::Unicode {
                name: &self.config.react_on_seen_emoji,
            };
            let _ = ctx.http.create_reaction(message.channel_id, message.id, &emoji).await;
        }

        // Une nouvelle réponse interrompt une fermeture programmée.
        if let (Some(_), Some(closer)) = (&thread.scheduled_close_at, &thread.scheduled_close_id) {
            store::cancel_scheduled_close(&self.db, &thread.id).await?;
            let mentions = parse_id(closer).map_or(Mentions::None, |id| Mentions::Explicit {
                users: vec![id],
                roles: Vec::new(),
                everyone: false,
            });
            self.post_system_message_with(
                ctx,
                thread,
                &format!("<@!{closer}> Le ticket dont la fermeture était programmée a reçu une nouvelle réponse. Fermeture annulée."),
                mentions,
                None,
            )
            .await?;
        }

        if !skip_alert {
            let current = store::find_by_id(&self.db, &thread.id).await?;
            if let Some(alert_ids) = current.and_then(|t| t.alert_ids) {
                let ids: Vec<&str> = alert_ids.split(',').collect();
                let mentions_text: String = ids.iter().map(|id| format!("<@!{id}> ")).collect();
                store::delete_alerts(&self.db, &thread.id).await?;
                let users = ids.iter().filter_map(|id| parse_id(id)).collect();
                self.post_system_message_with(
                    ctx,
                    thread,
                    &format!("{mentions_text}Nouveau message de {}", thread.user_name),
                    Mentions::Explicit { users, roles: Vec::new(), everyone: false },
                    None,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Ferme le ticket et supprime son salon.
    pub async fn close_thread(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        suppress_system_message: bool,
        silent: bool,
    ) -> Result<()> {
        if !suppress_system_message {
            tracing::info!(thread = %thread.id, "fermeture du ticket");
            self.post_system_message(
                ctx,
                thread,
                if silent { "Fermeture silencieuse du ticket…" } else { "Fermeture du ticket…" },
            )
            .await?;
        }
        store::set_status(&self.db, &thread.id, thread_status::CLOSED).await?;

        if let Some(channel_id) = thread.channel_id.as_deref().and_then(parse_id::<ChannelMarker>) {
            // Fil de forum : archivé et verrouillé (l'historique reste visible sur Discord).
            // Salon classique (ou ancien ticket d'avant le mode forum) : supprimé.
            let archived = self.config.new_thread_forum_id.is_some()
                && ctx.http.update_thread(channel_id).archived(true).locked(true).await.is_ok();
            if !archived
                && let Err(error) = ctx.http.delete_channel(channel_id).reason("Ticket fermé").await
                && !is_not_found(&error)
            {
                tracing::warn!(%error, "suppression du salon du ticket impossible");
            }
        }
        if let Some(handle) = self.auto_alerts.lock().expect("auto_alerts").remove(&thread.id) {
            handle.abort();
        }

        // Équivalent du hook afterThreadClose de logs.js.
        if let Some(closed) = store::find_by_id(&self.db, &thread.id).await? {
            self.save_log_to_storage(&closed).await?;
        }
        Ok(())
    }

    pub async fn edit_staff_reply(
        &self,
        ctx: &Ctx,
        thread: &Thread,
        original: &ThreadMessage,
        new_text: &str,
    ) -> Result<bool> {
        let edited = ThreadMessage { body: Some(new_text.to_owned()), ..original.clone() };
        let thread_content = formatters::staff_reply_thread_message(&self.config, &edited);
        let dm_content = formatters::staff_reply_dm(&self.config, &edited);
        if dm_content.encode_utf16().count() > 2000 || thread_content.encode_utf16().count() > 2000 {
            self.post_system_message(ctx, thread, "Réponse modifiée trop longue ! Elle doit faire moins de 2000 caractères au total, nom du modérateur compris.")
                .await?;
            return Ok(false);
        }

        if let (Some(channel), Some(id)) =
            (original.dm_channel_id.as_deref().and_then(parse_id), original.dm_message_id.as_deref().and_then(parse_id))
        {
            ctx.http.update_message(channel, id).content(Some(&dm_content)).await?;
        }
        if let (Some(channel), Some(id)) =
            (thread.channel_id.as_deref().and_then(parse_id), original.inbox_message_id.as_deref().and_then(parse_id))
        {
            ctx.http.update_message(channel, id).content(Some(&thread_content)).await?;
        }

        let notification = formatters::staff_reply_edit_notification(original, new_text);
        let inbox = self.post_to_thread(ctx, thread, Post::text(notification)).await?;
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::REPLY_EDITED,
                inbox_message_id: inbox.map(|m| m.id.to_string()),
                metadata: Some(json!({ "originalThreadMessage": message_json(original), "newBody": new_text })),
                ..Default::default()
            },
        )
        .await?;
        store::set_message_body(&self.db, original.id, new_text).await?;
        Ok(true)
    }

    pub async fn delete_staff_reply(&self, ctx: &Ctx, thread: &Thread, original: &ThreadMessage) -> Result<()> {
        if let (Some(channel), Some(id)) =
            (original.dm_channel_id.as_deref().and_then(parse_id), original.dm_message_id.as_deref().and_then(parse_id))
        {
            ctx.http.delete_message(channel, id).await?;
        }
        if let (Some(channel), Some(id)) =
            (thread.channel_id.as_deref().and_then(parse_id), original.inbox_message_id.as_deref().and_then(parse_id))
        {
            ctx.http.delete_message(channel, id).await?;
        }

        let notification = formatters::staff_reply_deletion_notification(original);
        let inbox = self.post_to_thread(ctx, thread, Post::text(notification)).await?;
        store::add_message(
            &self.db,
            &thread.id,
            NewMessage {
                message_type: message_type::REPLY_DELETED,
                inbox_message_id: inbox.map(|m| m.id.to_string()),
                metadata: Some(json!({ "originalThreadMessage": message_json(original) })),
                ..Default::default()
            },
        )
        .await?;
        store::delete_message(&self.db, original.id).await?;
        Ok(())
    }

    /// Récupère les messages envoyés par l'utilisateur pendant que le bot était hors ligne.
    pub async fn recover_downtime_messages(&self, ctx: &Ctx, thread: &Thread) -> Result<()> {
        if store::is_blocked(&self.db, &thread.user_id).await? {
            return Ok(());
        }
        let Some(user_id) = parse_id::<UserMarker>(&thread.user_id) else {
            return Ok(());
        };
        let Some(latest) = store::latest_user_exchange(&self.db, &thread.id).await? else {
            return Ok(());
        };
        let Some(after) = latest.dm_message_id.as_deref().and_then(parse_id::<MessageMarker>) else {
            return Ok(());
        };
        let channel = self.dm_channel(ctx, user_id).await?;
        let mut messages = ctx.http.channel_messages(channel).after(after).limit(50).await?.models().await?;
        // Discord renvoie du plus récent au plus ancien.
        messages.reverse();
        messages.retain(|m| m.author.id == user_id);
        if messages.is_empty() {
            return Ok(());
        }

        self.post_system_message(
            ctx,
            thread,
            &format!(
                "📥 Récupération de {} message(s) envoyé(s) par l'utilisateur pendant l'indisponibilité du bot !",
                messages.len()
            ),
        )
        .await?;
        for (index, message) in messages.iter().enumerate() {
            self.receive_user_reply(ctx, thread, message, index > 0).await?;
        }
        Ok(())
    }

    async fn main_guild_members(&self, ctx: &Ctx, user_id: Id<UserMarker>) -> Vec<(Id<GuildMarker>, Member)> {
        let mut members = Vec::new();
        for guild_id in &self.config.main_server_ids {
            if let Ok(response) = ctx.http.guild_member(*guild_id, user_id).await
                && let Ok(member) = response.model().await
            {
                members.push((*guild_id, member));
            }
        }
        members
    }

    /// Ouvre un nouveau ticket pour l'utilisateur. `Ok(None)` si les conditions
    /// (âge du compte, ancienneté sur le serveur) ne sont pas remplies.
    pub async fn create_new_thread_for_user(
        &self,
        ctx: &Ctx,
        user: &User,
        options: NewThreadOptions,
    ) -> Result<Option<Thread>> {
        let _guard = self.creation_lock.lock().await;
        if store::find_open_by_user_id(&self.db, &user.id.to_string()).await?.is_some() {
            anyhow::bail!("Tentative de création d'un ticket pour un utilisateur qui en a déjà un ouvert !");
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let created_ms = snowflake_timestamp_ms(user.id.get());

        if let (Some(hours), false) = (self.config.required_account_age_hours, options.ignore_requirements)
            && created_ms > now_ms - (hours * 3_600_000.0) as i64
        {
            if let Some(message) = &self.config.account_age_denied_message
                && let Ok(channel) = self.dm_channel(ctx, user.id).await
            {
                let _ = ctx.http.create_message(channel).content(message).await;
            }
            return Ok(None);
        }

        let guild_members = self.main_guild_members(ctx, user.id).await;

        // Sans donnée sur l'utilisateur dans les serveurs principaux, on lui laisse le bénéfice du doute.
        if let (Some(minutes), false) = (self.config.required_time_on_server_minutes, options.ignore_requirements) {
            let threshold = now_ms - (minutes * 60_000.0) as i64;
            let allowed = guild_members.is_empty()
                || guild_members.iter().any(|(_, m)| m.joined_at.is_some_and(|j| j.as_micros() / 1000 < threshold));
            if !allowed {
                if let Some(message) = &self.config.time_on_server_denied_message
                    && let Ok(channel) = self.dm_channel(ctx, user.id).await
                {
                    let _ = ctx.http.create_message(channel).content(message).await;
                }
                return Ok(None);
            }
        }

        let mut channel_name = slugify(&user.name);
        if channel_name.is_empty() {
            channel_name = "unknown".to_owned();
        }
        if self.config.anonymize_channel_name {
            channel_name = uuid::Uuid::new_v4().simple().to_string()[..12].to_owned();
        }
        tracing::info!(channel = %channel_name, "création d'un salon de ticket");

        // Catégorie : par serveur d'origine, sinon catégorie générale.
        let category = self
            .config
            .new_thread_category_from_server
            .iter()
            .find(|(guild, _)| guild_members.iter().any(|(id, _)| id.to_string() == *guild))
            .map(|(_, category)| category.clone())
            .or_else(|| self.config.new_thread_category_id.clone())
            .and_then(|id| parse_id::<ChannelMarker>(&id));

        let create = |name: String| {
            let http = &ctx.http;
            let inbox = self.config.inbox_server_id;
            async move {
                let mut request = http.create_guild_channel(inbox, &name).kind(ChannelType::GuildText);
                if let Some(category) = category {
                    request = request.parent_id(category);
                }
                request.reason("Nouveau ticket modmail").await
            }
        };
        let channel_id = if let Some(forum) = self.config.new_thread_forum_id {
            // Fil de forum : le message d'ouverture est obligatoire, l'en-tête suit.
            let starter = format!("Nouveau ticket : **{}**", escape_markdown(&user.name));
            let post = ctx
                .http
                .create_forum_thread(forum, &channel_name)
                .auto_archive_duration(AutoArchiveDuration::Week)
                .message()
                .content(&starter)
                .await?
                .model()
                .await?;
            post.channel.id
        } else {
            match create(channel_name).await {
                Ok(response) => response.model().await?.id,
                Err(error)
                    if error.to_string().contains("Contains words not allowed for servers in Server Discovery") =>
                {
                    create("badname-0000".to_owned()).await?.model().await?.id
                }
                Err(error) => return Err(error.into()),
            }
        };

        let thread_id =
            store::create_thread(&self.db, &user.id.to_string(), &user.name, &channel_id.to_string()).await?;
        let thread =
            store::find_by_id(&self.db, &thread_id).await?.ok_or_else(|| anyhow!("ticket {thread_id} introuvable"))?;

        if !options.quiet {
            let (mention, mentions) = self.inbox_mention();
            if !mention.trim().is_empty() {
                self.post_non_log_message(
                    ctx,
                    &thread,
                    &format!("{mention}Nouveau ticket modmail ({})", thread.user_name),
                    mentions,
                )
                .await?;
            }
        }

        // En-tête : âge du compte, identifiant, infos membre, historique.
        let account_age = humanize_duration(now_ms - created_ms, 2, None);
        let mut header = format!("ÂGE DU COMPTE **{account_age}**, ");
        if self.config.mention_user_in_thread_header {
            header.push_str(&format!("ID **{}** (<@!{}>)", user.id, user.id));
        } else {
            header.push_str(&format!("ID **{}**", user.id));
        }

        for (guild_id, member) in &guild_members {
            let nickname = member.nick.clone().unwrap_or_else(|| self.display_name(&member.user));
            let mut items = vec![format!("PSEUDO **{}**", escape_markdown(&nickname))];
            if let Some(joined) = member.joined_at {
                items.push(format!(
                    "ARRIVÉ IL Y A **{}**",
                    humanize_duration(now_ms - joined.as_micros() / 1000, 2, None)
                ));
            }
            if let Some(voice) = ctx.cache.voice_state(user.id, *guild_id)
                && let Some(channel) = ctx.cache.channel(voice.channel_id())
                && let Some(name) = &channel.name
            {
                items.push(format!("SALON VOCAL **{}**", escape_markdown(name)));
            }
            if self.config.roles_in_thread_header && !member.roles.is_empty() {
                let names: Vec<String> = member
                    .roles
                    .iter()
                    .filter_map(|id| ctx.cache.role(*id).map(|r| r.resource().name.clone()))
                    .collect();
                items.push(format!("RÔLES **{}**", names.join(", ")));
            }
            let line = items.join(", ");
            if self.config.main_server_ids.len() == 1 {
                header.push_str(&format!("\n{line}"));
            } else {
                let guild_name =
                    ctx.cache.guild(*guild_id).map(|g| g.name().to_owned()).unwrap_or_else(|| guild_id.to_string());
                header.push_str(&format!("\n**[{}]** {line}", escape_markdown(&guild_name)));
            }
        }

        let previous = store::closed_thread_count(&self.db, &user.id.to_string()).await?;
        if previous > 0 {
            header.push_str(&format!(
                "\n\nCet utilisateur a **{previous}** ticket(s) précédent(s). Utilise `{}logs` pour les voir.",
                self.config.prefix
            ));
        }
        let notes = store::notes_by_user(&self.db, &user.id.to_string()).await?;
        if !notes.is_empty() {
            header.push_str(&format!(
                "\n\nCet utilisateur a **{}** note(s). Utilise `{}notes` pour les voir.",
                notes.len(),
                self.config.prefix
            ));
        }
        header.push_str("\n────────────────");

        let mentions = if self.config.mention_user_in_thread_header {
            Mentions::Explicit { users: vec![user.id], roles: Vec::new(), everyone: false }
        } else {
            Mentions::None
        };
        let controls = super::buttons::controls(&thread.id, None);
        let header_message = self
            .post_system_message_full(
                ctx,
                &thread,
                &header,
                Post { mentions, components: controls, ..Default::default() },
            )
            .await?;
        if self.config.pin_thread_header
            && let Some(message) = header_message
        {
            let _ = ctx.http.create_pin(message.channel_id, message.id).await;
        }
        Ok(Some(thread))
    }

    /// Mention du staff (`mentionRole`) et mentions autorisées correspondantes.
    pub fn inbox_mention(&self) -> (String, Mentions) {
        mention_roles(&self.config.mention_role)
    }
}

/// `mentionRole` : identifiants de rôles, `here`, `everyone`, ou `none`/`off`.
pub fn mention_roles(roles: &[String]) -> (String, Mentions) {
    let valid: Vec<&String> = roles.iter().filter(|r| !matches!(r.as_str(), "" | "none" | "off")).collect();
    if valid.is_empty() {
        return (String::new(), Mentions::None);
    }
    let mut parts = Vec::new();
    let mut role_ids = Vec::new();
    let mut everyone = false;
    for role in valid {
        match role.as_str() {
            "here" => {
                parts.push("@here".to_owned());
                everyone = true;
            }
            "everyone" => {
                parts.push("@everyone".to_owned());
                everyone = true;
            }
            id => {
                parts.push(format!("<@&{id}>"));
                if let Some(role_id) = parse_id(id) {
                    role_ids.push(role_id);
                }
            }
        }
    }
    (format!("{} ", parts.join(" ")), Mentions::Explicit { users: Vec::new(), roles: role_ids, everyone })
}

/// Sérialisation d'un message pour les métadonnées (`getSQLProps` de modmailbot).
fn message_json(message: &ThreadMessage) -> Value {
    json!({
        "id": message.id,
        "thread_id": message.thread_id,
        "message_type": message.message_type,
        "message_number": message.message_number,
        "user_id": message.user_id,
        "user_name": message.user_name,
        "role_name": message.role_name,
        "body": message.body,
        "is_anonymous": message.is_anonymous,
        "dm_channel_id": message.dm_channel_id,
        "dm_message_id": message.dm_message_id,
        "inbox_message_id": message.inbox_message_id,
        "created_at": message.created_at,
    })
}
