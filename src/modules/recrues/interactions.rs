use super::{
    COLOR, MOTIF_MAX_LENGTH, NAMESPACE, VETO_COLOR, Vote,
    embed::{format_entry_detail, format_tally},
    service,
};
use crate::{
    discord::{
        commands::{modal_select, modal_text},
        components::{emoji, label, select_option, string_select, text_input},
        embed::EmbedBuilder,
        interaction::{Ix, Reply},
        members::fetch_display_name,
        messages::parse_id,
    },
    modules::permissions::{self, Perm},
    util::{
        format::{field, mention},
        ids::{CustomId, encode_custom_id},
    },
};
use anyhow::Result;
use twilight_model::{
    application::interaction::modal::ModalInteractionData,
    channel::message::{Component, component::TextInputStyle},
};

const ENTRY_GONE: &str = "Cette recrue n'est plus suivie (la semaine a peut-être été réinitialisée).";

fn motif_input() -> Component {
    let mut input = text_input("motif", TextInputStyle::Paragraph);
    input.max_length = Some(MOTIF_MAX_LENGTH);
    Component::TextInput(input)
}

/// Modal du vote : un menu « Pour / Contre / Blanc » et un motif obligatoire.
fn vote_modal(display_name: &str) -> (String, Vec<Component>) {
    let options = Vote::ORDER
        .iter()
        .map(|vote| {
            let mut option = select_option(vote.label(), vote.as_str());
            option.emoji = Some(emoji(vote.emoji()));
            option
        })
        .collect();
    let mut select = string_select("choix".to_owned(), "Choisis ton vote", options);
    select.required = Some(true);
    (
        format!("Avis sur {display_name}"),
        vec![
            label("Vote", None, Component::SelectMenu(select)),
            label("Motif du vote", Some("Explique pourquoi — visible par les gradés."), motif_input()),
        ],
    )
}

fn veto_modal(display_name: &str) -> (String, Vec<Component>) {
    (
        format!("Véto sur {display_name}"),
        vec![label("Motif du véto", Some("Obligatoire — le véto bloque la validation de la recrue."), motif_input())],
    )
}

pub async fn select(ix: &Ix, custom_id: &CustomId<'_>, values: &[String]) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return Ok(());
    };
    let Some(entry_id) = values.first() else {
        return Ok(());
    };
    let guild = guild_id.to_string();
    let Some((entry, session)) = service::get_entry(&ix.ctx.db, &guild, entry_id).await? else {
        return ix.reply(Reply::ephemeral(ENTRY_GONE)).await;
    };
    let display_name = async {
        match parse_id(&entry.user_id) {
            Some(user_id) => {
                fetch_display_name(&ix.ctx, guild_id, user_id).await.unwrap_or_else(|| "cette recrue".to_owned())
            }
            None => "cette recrue".to_owned(),
        }
    };

    match custom_id.action {
        "detail" => {
            if !permissions::require(ix, Perm::RecrueSuivi).await? {
                return Ok(());
            }
            let embed = EmbedBuilder::new()
                .title("Avis détaillés")
                .color(if entry.veto_by_id.is_some() { VETO_COLOR } else { COLOR })
                .description(format!("{}\n{}", mention(&entry.user_id), format_tally(&entry.votes)))
                .field(field("Détail", format_entry_detail(&entry), false))
                .build();
            ix.reply(Reply::embed(embed).hidden()).await
        }
        "vote" => {
            if !permissions::require(ix, Perm::RecrueAvis).await? {
                return Ok(());
            }
            if session.closed_at.is_some() {
                return ix.reply(Reply::ephemeral("Cette semaine d'avis est clôturée.")).await;
            }
            let (title, components) = vote_modal(&display_name.await);
            ix.modal(encode_custom_id(NAMESPACE, "vote-modal", &entry.id, None), title, components).await
        }
        "veto" => {
            if !permissions::require(ix, Perm::RecrueVeto).await? {
                return Ok(());
            }
            if let Some(by) = &entry.veto_by_id {
                return ix
                    .reply(Reply::ephemeral(format!(
                        "Cette recrue est déjà sous véto de {}. Utilise `/recrue veto lever` pour le retirer.",
                        mention(by)
                    )))
                    .await;
            }
            let (title, components) = veto_modal(&display_name.await);
            ix.modal(encode_custom_id(NAMESPACE, "veto-modal", &entry.id, None), title, components).await
        }
        _ => Ok(()),
    }
}

pub async fn modal(ix: &Ix, action: &str, entry_id: &str, modal: &ModalInteractionData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return Ok(());
    };
    let guild = guild_id.to_string();
    let Some((entry, _)) = service::get_entry(&ix.ctx.db, &guild, entry_id).await? else {
        return ix.reply(Reply::ephemeral(ENTRY_GONE)).await;
    };

    let motif = modal_text(modal, "motif").trim().to_owned();
    if motif.is_empty() {
        return ix.reply(Reply::ephemeral("Le motif est obligatoire.")).await;
    }

    match action {
        "vote-modal" => {
            if !permissions::require(ix, Perm::RecrueAvis).await? {
                return Ok(());
            }
            let Some(vote) = modal_select(modal, "choix").first().and_then(|v| Vote::parse(v)) else {
                return ix.reply(Reply::ephemeral("Vote invalide.")).await;
            };
            service::cast_vote(&ix.ctx.db, &entry.id, &ix.user_id().to_string(), vote, &motif).await?;
            ix.defer_update().await?;
            service::sync_dashboard(&ix.ctx, guild_id).await?;
            let warning =
                if entry.veto_by_id.is_some() { "\n⛔ Attention : cette recrue est sous véto." } else { "" };
            ix.followup(Reply::ephemeral(format!(
                "{} Avis « {} » enregistré pour {}.{warning}",
                vote.emoji(),
                vote.label(),
                mention(&entry.user_id)
            )))
            .await
        }
        "veto-modal" => {
            if !permissions::require(ix, Perm::RecrueVeto).await? {
                return Ok(());
            }
            service::set_veto(&ix.ctx.db, &entry.id, &ix.user_id().to_string(), &motif).await?;
            ix.defer_update().await?;
            service::sync_dashboard(&ix.ctx, guild_id).await?;
            ix.followup(Reply::ephemeral(format!("⛔ Véto posé sur {}.", mention(&entry.user_id)))).await
        }
        _ => Ok(()),
    }
}
