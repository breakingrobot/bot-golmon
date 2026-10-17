//! Actions au clic droit liées aux tickets : « Ouvrir un ticket » sur un membre
//! (staff) et « Signaler au staff » sur un message (tout le monde).

use super::{Modmail, store, thread::NewThreadOptions, util::escape_markdown};
use crate::discord::interaction::{Ix, Reply};
use anyhow::Result;
use twilight_model::{channel::Message, user::User};

impl Modmail {
    /// Ouvre (ou retrouve) le ticket d'un membre, comme `!newthread`.
    pub async fn open_ticket_for(&self, ix: &Ix, user: &User) -> Result<()> {
        if !self.is_staff_member(&ix.ctx, ix.user_id(), ix.member_roles()) {
            return ix.reply(Reply::ephemeral("Réservé au staff des tickets.")).await;
        }
        if user.bot {
            return ix.reply(Reply::ephemeral("Impossible d'ouvrir un ticket pour un bot.")).await;
        }
        if let Some(existing) = store::find_open_by_user_id(&self.db, &user.id.to_string()).await? {
            let channel = existing.channel_id.unwrap_or_default();
            return ix
                .reply(Reply::ephemeral(format!("Un ticket est déjà ouvert avec ce membre : <#{channel}>")))
                .await;
        }
        ix.defer(true).await?;
        let Some(created) = self
            .create_new_thread_for_user(&ix.ctx, user, NewThreadOptions { quiet: true, ignore_requirements: true })
            .await?
        else {
            return ix.edit_text("Le ticket n'a pas pu être ouvert.").await;
        };
        let opener = ix.interaction.member.as_ref().and_then(|m| m.nick.clone()).unwrap_or_else(|| ix.username());
        self.post_system_message(&ix.ctx, &created, &format!("Ticket ouvert par {opener}")).await?;
        ix.edit_text(format!("Ticket ouvert : <#{}>", created.channel_id.unwrap_or_default())).await
    }

    /// Transmet un message au staff dans le ticket de la personne qui signale
    /// (ouvert au besoin). Le signalement n'est visible que du staff.
    pub async fn report_message(&self, ix: &Ix, message: &Message) -> Result<()> {
        let Some(reporter) = ix.user().cloned() else {
            return Ok(());
        };
        if store::is_blocked(&self.db, &reporter.id.to_string()).await? {
            return ix.reply(Reply::ephemeral("Tu ne peux pas contacter le staff pour le moment.")).await;
        }
        ix.defer(true).await?;
        let thread = match store::find_open_by_user_id(&self.db, &reporter.id.to_string()).await? {
            Some(thread) => thread,
            None => match self.create_new_thread_for_user(&ix.ctx, &reporter, NewThreadOptions::default()).await? {
                Some(thread) => thread,
                None => return ix.edit_text("Ton signalement n'a pas pu être transmis.").await,
            },
        };

        let link = match message.guild_id.or(ix.guild_id()) {
            Some(guild) => format!("https://discord.com/channels/{guild}/{}/{}", message.channel_id, message.id),
            None => String::new(),
        };
        let mut content: String = message.content.chars().take(1500).collect();
        if content.len() < message.content.len() {
            content.push('…');
        }
        let quoted = if content.is_empty() {
            "*(message sans texte)*".to_owned()
        } else {
            content.lines().map(|line| format!("> {line}")).collect::<Vec<_>>().join("\n")
        };
        let attachments: Vec<String> = message.attachments.iter().map(|a| a.url.clone()).collect();
        let mut text = format!(
            "🚩 **Signalement** de <@{}> : message de **{}** (<@{}>) dans <#{}>\n{quoted}",
            reporter.id,
            escape_markdown(&message.author.name),
            message.author.id,
            message.channel_id,
        );
        if !attachments.is_empty() {
            text.push_str(&format!("\nPièces jointes : {}", attachments.join(" ")));
        }
        if !link.is_empty() {
            text.push_str(&format!("\n{link}"));
        }
        self.post_system_message(&ix.ctx, &thread, &text).await?;
        ix.edit_text("Merci, ton signalement a été transmis au staff. Il pourra te répondre en message privé.").await
    }
}
