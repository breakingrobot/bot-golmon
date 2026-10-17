use super::{model::ResponseStatus, service};
use crate::{
    discord::{
        commands::modal_text,
        components::{label, text_input},
        interaction::{GUILD_ONLY, Ix, Reply},
        messages::{self, Mentions, Outgoing},
    },
    util::{
        format::chunk_mentions,
        ids::{CustomId, encode_custom_id},
    },
};
use anyhow::Result;
use twilight_model::{
    application::interaction::modal::ModalInteractionData,
    channel::message::{Component, component::TextInputStyle},
};

pub async fn button(ix: &Ix, custom_id: &CustomId<'_>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let Some(presence) = service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), custom_id.id).await? else {
        return ix.reply(Reply::ephemeral("Présence introuvable.")).await;
    };
    if presence.closed_at.is_some() {
        return ix.reply(Reply::ephemeral("Cette présence est clôturée.")).await;
    }

    let status = match custom_id.action {
        "present" => Some(ResponseStatus::Present),
        "absent" => Some(ResponseStatus::Absent),
        _ => None,
    };
    if let Some(status) = status {
        let changed =
            service::register_response(&ix.ctx.db, &presence.id, &ix.user_id().to_string(), status, None).await?;
        ix.defer_update().await?;
        // Rien n'a bougé : inutile de rééditer le message.
        if changed {
            service::sync_message(&ix.ctx, &presence.id).await?;
        }
        return Ok(());
    }

    match custom_id.action {
        "retard" => {
            let mut input = text_input("minutes", TextInputStyle::Short);
            input.placeholder = Some("15".to_owned());
            ix.modal(
                encode_custom_id("presence", "retard-modal", &presence.id, None),
                "Retard".to_owned(),
                vec![label("Durée du retard (minutes)", None, Component::TextInput(input))],
            )
            .await
        }
        "ping" => {
            ix.defer(true).await?;
            let ids = service::non_respondent_ids(&ix.ctx, &presence).await?;
            if ids.is_empty() {
                return ix.edit_text("Tout le monde a répondu.").await;
            }
            if let Some(channel_id) = ix.channel_id() {
                for content in chunk_mentions(&ids, &format!("⏰ Rappel — **{}**", presence.titre)) {
                    messages::send(&ix.ctx, channel_id, &Outgoing::text(content).with_mentions(Mentions::Users))
                        .await?;
                }
            }
            ix.edit_text(format!("{} membre(s) relancé(s).", ids.len())).await
        }
        _ => Ok(()),
    }
}

pub async fn retard_modal(ix: &Ix, presence_id: &str, modal: &ModalInteractionData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let minutes = match modal_text(modal, "minutes").trim().parse::<i64>() {
        Ok(minutes) if minutes >= 0 => minutes,
        _ => return ix.reply(Reply::ephemeral("Durée invalide, indique un nombre de minutes.")).await,
    };

    let presence = service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), presence_id).await?;
    let Some(presence) = presence.filter(|p| p.closed_at.is_none()) else {
        return ix.reply(Reply::ephemeral("Présence introuvable ou clôturée.")).await;
    };

    let changed = service::register_response(
        &ix.ctx.db,
        &presence.id,
        &ix.user_id().to_string(),
        ResponseStatus::Retard,
        Some(minutes),
    )
    .await?;
    ix.defer_update().await?;
    if changed {
        service::sync_message(&ix.ctx, &presence.id).await?;
    }
    Ok(())
}
