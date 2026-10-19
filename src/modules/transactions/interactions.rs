use super::{commands::CLOSED, model::Status, service};
use crate::{
    discord::{
        commands::modal_text,
        components::{label, text_input},
        interaction::{GUILD_ONLY, Ix, Reply},
        messages,
    },
    modules::permissions::{self, Perm},
    util::ids::{CustomId, encode_custom_id},
};
use anyhow::Result;
use twilight_model::{
    application::interaction::modal::ModalInteractionData,
    channel::message::{Component, component::TextInputStyle},
};

fn input(custom_id: &str, placeholder: &str, required: bool) -> Component {
    let mut input = text_input(custom_id, TextInputStyle::Short);
    input.placeholder = Some(placeholder.to_owned());
    input.required = Some(required);
    Component::TextInput(input)
}

pub async fn button(ix: &Ix, custom_id: &CustomId<'_>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    let Some(transaction) = service::get(&ix.ctx.db, &guild, custom_id.id).await? else {
        return ix.reply(Reply::ephemeral("Transaction introuvable.")).await;
    };
    if transaction.status() == Status::Cloturee {
        return ix.reply(Reply::ephemeral(CLOSED)).await;
    }
    let perm = if custom_id.action == "delete" { Perm::TransactionSupprimer } else { Perm::TransactionUtiliser };
    if !permissions::require(ix, perm).await? {
        return Ok(());
    }

    match custom_id.action {
        "additem" => {
            ix.modal(
                encode_custom_id("transaction", "additem-modal", &transaction.id, None),
                "Ajouter un objet".to_owned(),
                vec![
                    label("Nom de l'objet", None, input("nom", "Arme, Voiture, etc.", true)),
                    label("Quantité", None, input("quantite", "1", false)),
                    label("Prix unitaire (optionnel)", None, input("prix_unitaire", "150000", false)),
                ],
            )
            .await
        }
        "delete" => {
            ix.defer_update().await?;
            if let (Some(channel_id), Some(message)) = (ix.channel_id(), ix.interaction.message.as_ref()) {
                messages::delete_quietly(&ix.ctx, channel_id, message.id).await;
            }
            service::delete(&ix.ctx.db, &guild, &transaction.id).await?;
            service::sync_dashboard(&ix.ctx, &guild).await?;
            ix.followup(Reply::ephemeral("Transaction supprimée.")).await
        }
        "status" => {
            let Some(status) = custom_id.payload.and_then(Status::parse) else {
                return ix.reply(Reply::ephemeral("Statut inconnu.")).await;
            };
            ix.defer_update().await?;
            service::set_status(&ix.ctx.db, &guild, &transaction.id, status).await?;
            service::refresh(&ix.ctx, &guild, &transaction.id).await?;
            ix.followup(Reply::ephemeral(format!("Statut passé à **{}**.", status.label()))).await
        }
        _ => ix.reply(Reply::ephemeral("Action non reconnue.")).await,
    }
}

pub async fn add_item_modal(ix: &Ix, transaction_id: &str, modal: &ModalInteractionData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    let nom = modal_text(modal, "nom").trim().to_owned();
    let quantite_raw = modal_text(modal, "quantite");
    let prix_raw = modal_text(modal, "prix_unitaire");

    let quantite = if quantite_raw.trim().is_empty() { Some(1) } else { quantite_raw.trim().parse::<i64>().ok() };
    let Some(quantite) = quantite.filter(|q| *q >= 1) else {
        return ix.reply(Reply::ephemeral("Quantité invalide.")).await;
    };
    let prix_unitaire = if prix_raw.trim().is_empty() {
        None
    } else {
        match prix_raw.trim().replace(',', ".").parse::<f64>() {
            Ok(value) if value.is_finite() => Some(value),
            _ => return ix.reply(Reply::ephemeral("Prix unitaire invalide.")).await,
        }
    };

    let Some(transaction) = service::get(&ix.ctx.db, &guild, transaction_id).await? else {
        return ix.reply(Reply::ephemeral("Transaction introuvable.")).await;
    };
    if transaction.status() == Status::Cloturee {
        return ix.reply(Reply::ephemeral(CLOSED)).await;
    }

    ix.defer_update().await?;
    // La transaction a pu être supprimée entre l'ouverture du modal et sa validation.
    if !service::add_item(&ix.ctx.db, &transaction.id, &nom, quantite, prix_unitaire).await? {
        return ix.followup(Reply::ephemeral("Transaction introuvable (elle a peut-être été supprimée).")).await;
    }
    service::refresh(&ix.ctx, &guild, &transaction.id).await?;
    ix.followup(Reply::ephemeral(format!("Objet « {nom} » ajouté."))).await
}
