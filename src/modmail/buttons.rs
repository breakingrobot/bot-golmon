//! Boutons du salon de ticket : prise en charge, réponse (signée ou anonyme)
//! par fenêtre de saisie, fermeture avec confirmation. Les commandes texte
//! (`!r`, `!close`…) restent disponibles.

use super::{
    Modmail,
    models::Thread,
    staff::{self, Claim},
    store,
    thread::{Moderator, StaffReply},
};
use crate::{
    discord::{
        commands::modal_text,
        components::{self, ButtonExt as _},
        interaction::{Ix, Reply},
        messages::Outgoing,
    },
    util::ids::{CustomId, encode_custom_id},
};
use anyhow::Result;
use twilight_model::{
    application::interaction::modal::ModalInteractionData,
    channel::message::{
        Component,
        component::{ButtonStyle, TextInputStyle},
    },
};

pub const NAMESPACE: &str = "ticket";

/// Rangée de boutons affichée sous l'en-tête du ticket.
pub fn controls(thread_id: &str, claim: Option<&Claim>) -> Vec<Component> {
    let id = |action: &str| encode_custom_id(NAMESPACE, action, thread_id, None);
    let claim_button = match claim {
        Some(claim) => {
            components::button(id("claim"), format!("Pris en charge : {}", claim.staff_name), ButtonStyle::Secondary)
                .with_emoji("🙋")
        }
        None => components::button(id("claim"), "Prendre en charge", ButtonStyle::Primary).with_emoji("🙋"),
    };
    vec![components::row(vec![
        Component::Button(claim_button),
        Component::Button(components::button(id("reply"), "Répondre", ButtonStyle::Success).with_emoji("✉️")),
        Component::Button(components::button(id("anon"), "Répondre anonymement", ButtonStyle::Secondary)),
        Component::Button(components::button(id("close"), "Fermer", ButtonStyle::Danger).with_emoji("🔒")),
    ])]
}

fn moderator(ix: &Ix) -> Option<Moderator> {
    let member = ix.interaction.member.as_ref()?;
    Some(Moderator { user: ix.user()?.clone(), nick: member.nick.clone(), roles: member.roles.clone() })
}

/// Module ticket actif, interaction sur le serveur inbox par un membre du staff.
async fn staff_modmail(ix: &Ix) -> Result<Option<&Modmail>> {
    let Some(modmail) = ix.ctx.modmail.as_ref() else {
        return Ok(None);
    };
    if ix.guild_id() != Some(modmail.config.inbox_server_id)
        || !modmail.is_staff_member(&ix.ctx, ix.user_id(), ix.member_roles())
    {
        ix.reply(Reply::ephemeral("Réservé au staff des tickets.")).await?;
        return Ok(None);
    }
    Ok(Some(modmail))
}

async fn open_thread(ix: &Ix, modmail: &Modmail, thread_id: &str) -> Result<Option<Thread>> {
    match store::find_by_id(&modmail.db, thread_id).await? {
        Some(thread) if thread.is_open() => Ok(Some(thread)),
        _ => {
            ix.notify("Ce ticket est fermé.").await?;
            Ok(None)
        }
    }
}

pub async fn button(ix: &Ix, custom_id: &CustomId<'_>) -> Result<()> {
    let Some(modmail) = staff_modmail(ix).await? else {
        return Ok(());
    };
    let Some(thread) = open_thread(ix, modmail, custom_id.id).await? else {
        return Ok(());
    };
    match custom_id.action {
        "claim" => claim(ix, modmail, &thread).await,
        "reply" | "anon" => {
            let anonymous = custom_id.action == "anon";
            let input = components::text_input("text", TextInputStyle::Paragraph);
            let input = twilight_model::channel::message::component::TextInput { max_length: Some(4000), ..input };
            let title = if anonymous { "Réponse anonyme" } else { "Réponse" };
            ix.modal(
                encode_custom_id(NAMESPACE, "reply-modal", &thread.id, Some(if anonymous { "anon" } else { "signed" })),
                format!("{title} à {}", thread.user_name),
                vec![components::label(
                    "Message",
                    Some("Envoyé en message privé à l'utilisateur."),
                    Component::TextInput(input),
                )],
            )
            .await
        }
        "close" => {
            let id = |action: &str| encode_custom_id(NAMESPACE, action, &thread.id, None);
            let mut reply =
                Reply::ephemeral(format!("Fermer le ticket #{} avec {} ?", thread.number(), thread.user_name));
            reply.message.components = vec![components::row(vec![
                Component::Button(components::button(id("close-confirm"), "Fermer", ButtonStyle::Danger)),
                Component::Button(components::button(
                    id("close-silent"),
                    "Fermer sans prévenir",
                    ButtonStyle::Secondary,
                )),
            ])];
            ix.reply(reply).await
        }
        "close-confirm" | "close-silent" => {
            ix.defer_update().await?;
            let name = moderator(ix).map(|m| modmail.display_name(&m.user)).unwrap_or_else(|| ix.username());
            modmail.close_now(&ix.ctx, &thread, &name, ix.user_id(), false, custom_id.action == "close-silent").await?;
            ix.edit(Reply { message: Outgoing::text("Ticket fermé."), ephemeral: true }).await
        }
        _ => Ok(()),
    }
}

async fn claim(ix: &Ix, modmail: &Modmail, thread: &Thread) -> Result<()> {
    ix.defer_update().await?;
    let db = &ix.ctx.db;
    let user_id = ix.user_id().to_string();
    let name = moderator(ix)
        .map(|m| m.nick.clone().unwrap_or_else(|| modmail.display_name(&m.user)))
        .unwrap_or_else(|| ix.username());

    let current = staff::claim_of(db, &thread.id).await?;
    let (claim, text) = match current {
        Some(current) if current.staff_id == user_id => {
            staff::release_claim(db, &thread.id).await?;
            (None, format!("**{name}** ne prend plus en charge ce ticket."))
        }
        previous => {
            staff::set_claim(db, &thread.id, &user_id, &name).await?;
            let text = match previous {
                Some(previous) => format!("Ticket repris par **{name}** (auparavant {}).", previous.staff_name),
                None => format!("Ticket pris en charge par **{name}**."),
            };
            (Some(Claim { thread_id: thread.id.clone(), staff_id: user_id, staff_name: name }), text)
        }
    };

    if let Some(message) = &ix.interaction.message {
        ix.ctx
            .http
            .update_message(message.channel_id, message.id)
            .components(Some(&controls(&thread.id, claim.as_ref())))
            .await?;
    }
    modmail.post_system_message(&ix.ctx, thread, &text).await?;
    Ok(())
}

pub async fn modal(ix: &Ix, custom_id: &CustomId<'_>, data: &ModalInteractionData) -> Result<()> {
    let Some(modmail) = staff_modmail(ix).await? else {
        return Ok(());
    };
    let Some(thread) = open_thread(ix, modmail, custom_id.id).await? else {
        return Ok(());
    };
    let Some(moderator) = moderator(ix) else {
        return Ok(());
    };
    let text = modal_text(data, "text");
    if text.trim().is_empty() {
        return ix.reply(Reply::ephemeral("Message vide.")).await;
    }
    ix.defer_update().await?;
    let anonymous = custom_id.payload == Some("anon");
    modmail
        .reply_to_user(
            &ix.ctx,
            &thread,
            &moderator,
            StaffReply { text: &text, attachments: &[], anonymous, reference: None },
        )
        .await?;
    Ok(())
}
