use super::{INVALID_FORMAT, NAMESPACE, format, is_valid, normalize, service};
use crate::{
    discord::{
        commands::modal_text,
        components::{label, text_input},
        interaction::{GUILD_ONLY, Ix, Reply},
    },
    util::ids::{CustomId, encode_custom_id},
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
    match custom_id.action {
        "add" => {
            let mut input = text_input("numero", TextInputStyle::Short);
            input.placeholder = Some("5551234".to_owned());
            ix.modal(
                encode_custom_id(NAMESPACE, "add-modal", &guild_id.to_string(), None),
                "Ajouter un numéro".to_owned(),
                vec![label("Numéro de téléphone", None, Component::TextInput(input))],
            )
            .await
        }
        "remove" => {
            ix.defer(true).await?;
            if !service::remove(&ix.ctx.db, &guild_id.to_string(), &ix.user_id().to_string()).await? {
                return ix.edit_text("Tu n'as pas de numéro enregistré.").await;
            }
            service::sync_dashboard(&ix.ctx, &guild_id.to_string()).await?;
            ix.edit_text("Ton numéro a été retiré.").await
        }
        _ => Ok(()),
    }
}

pub async fn add_modal(ix: &Ix, modal: &ModalInteractionData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let numero = normalize(&modal_text(modal, "numero"));
    if !is_valid(&numero) {
        return ix.reply(Reply::ephemeral(INVALID_FORMAT)).await;
    }
    ix.defer_update().await?;
    if !service::set(&ix.ctx.db, &guild_id.to_string(), &ix.user_id().to_string(), &numero).await? {
        return ix.followup(Reply::ephemeral(service::NUMBER_TAKEN)).await;
    }
    service::sync_dashboard(&ix.ctx, &guild_id.to_string()).await?;
    ix.followup(Reply::ephemeral(format!("Numéro enregistré : `{}`", format(&numero)))).await
}
