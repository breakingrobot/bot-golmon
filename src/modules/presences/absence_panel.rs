//! Panneau des absences : un message permanent avec les boutons « Déclarer une
//! absence » (formulaire) et « Terminer mon absence ».

use super::{
    commands::{AbsenceForm, declare_absence, end_absence},
    service,
};
use crate::{
    discord::{
        commands::modal_text,
        components::{self, ButtonExt as _, label, text_input},
        embed::EmbedBuilder,
        interaction::{GUILD_ONLY, Ix, Reply},
        members::avatar_url,
        messages::{self, Outgoing, parse_id},
        options::Options,
    },
    modules::permissions::{self, Perm},
    util::ids::{CustomId, encode_custom_id},
};
use anyhow::Result;
use twilight_model::{
    application::interaction::modal::ModalInteractionData,
    channel::{
        ChannelType,
        message::{
            Component,
            component::{ButtonStyle, TextInputStyle},
        },
    },
};
use twilight_util::builder::command::{ChannelBuilder, SubCommandBuilder};

pub const NAMESPACE: &str = "absence";

/// `/gestion absence panneau`.
pub fn admin_subcommands() -> Vec<SubCommandBuilder> {
    vec![
        SubCommandBuilder::new("panneau", "Publie le panneau « Déclarer une absence »").option(
            ChannelBuilder::new("salon", "Salon du panneau (défaut : salon des absences, sinon ce salon)")
                .channel_types([ChannelType::GuildText]),
        ),
    ]
}

pub async fn handle_admin(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !permissions::require(ix, Perm::AbsenceAutrui).await? {
        return Ok(());
    }
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    let channel_id = options
        .channel("salon")
        .or_else(|| config.and_then(|c| c.absence_channel_id).as_deref().and_then(parse_id))
        .or_else(|| ix.channel_id());
    let Some(channel_id) = channel_id else {
        return ix.reply(Reply::ephemeral("Salon introuvable.")).await;
    };

    let embed = EmbedBuilder::new()
        .title("🌙 Absences")
        .color(0xe67e22)
        .description(
            "Tu seras absent ? Déclare-le ici : le rôle d'absence est posé automatiquement, tu es marqué \
             absent aux présences de la période et personne ne te pingue.\n\n\
             Dates au format `JJ/MM/AAAA`. Une absence couvre des journées entières.",
        )
        .build();
    let id = |action: &str| encode_custom_id(NAMESPACE, action, "panel", None);
    let buttons = components::row(vec![
        Component::Button(
            components::button(id("declare"), "Déclarer une absence", ButtonStyle::Primary).with_emoji("🌙"),
        ),
        Component::Button(components::button(id("end"), "Terminer / annuler mon absence", ButtonStyle::Secondary)),
    ]);
    messages::send(&ix.ctx, channel_id, &Outgoing::embeds(vec![embed]).with_components(vec![buttons])).await?;
    ix.reply(Reply::ephemeral(format!("Panneau publié dans <#{channel_id}>."))).await
}

pub async fn button(ix: &Ix, custom_id: &CustomId<'_>) -> Result<()> {
    match custom_id.action {
        "declare" => {
            let short = |id: &str, placeholder: &str, required: bool| {
                let mut input = text_input(id, TextInputStyle::Short);
                input.placeholder = Some(placeholder.to_owned());
                input.required = Some(required);
                input.max_length = Some(if id == "motif" { 200 } else { 10 });
                Component::TextInput(input)
            };
            ix.modal(
                encode_custom_id(NAMESPACE, "declare-modal", "panel", None),
                "Déclarer une absence".to_owned(),
                vec![
                    label("Motif", None, short("motif", "Vacances, travail…", true)),
                    label("Dernier jour d'absence", Some("JJ/MM/AAAA"), short("fin", "31/12/2026", true)),
                    label("Premier jour", Some("JJ/MM/AAAA, aujourd'hui si vide"), short("debut", "", false)),
                ],
            )
            .await
        }
        "end" => end_absence(ix, ix.user_id()).await,
        _ => Ok(()),
    }
}

pub async fn modal(ix: &Ix, data: &ModalInteractionData) -> Result<()> {
    let form = AbsenceForm {
        target: ix.user_id(),
        motif: modal_text(data, "motif"),
        debut: Some(modal_text(data, "debut")),
        fin: modal_text(data, "fin"),
        avatar: ix.user().map(avatar_url),
    };
    declare_absence(ix, form).await
}
