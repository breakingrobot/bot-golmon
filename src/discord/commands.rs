//! Déclaration des slash commands et aiguillage des interactions.

use crate::{
    context::{Context, Ctx},
    discord::{interaction::Ix, options::Options},
    modules::{phones, presences, recrues, transactions, vehicles},
    util::ids::decode_custom_id,
};
use anyhow::Result;
use twilight_model::application::{
    command::{Command, CommandType},
    interaction::{
        Interaction, InteractionData, InteractionType,
        modal::{ModalInteractionComponent, ModalInteractionData},
    },
};

pub fn definitions(with_tickets: bool) -> Vec<Command> {
    let mut commands = vec![
        presences::commands::presence_definition(),
        presences::commands::absence_definition(),
        transactions::commands::definition(),
        vehicles::commands::definition(),
        recrues::commands::definition(),
        phones::commands::definition(),
        crate::modules::cotisations::commands::definition(),
        super::admin::config_definition(),
        super::admin::gestion_definition(),
    ];
    commands.extend(super::context_menus::definitions(with_tickets));
    commands
}

/// Enregistre les commandes : par guilde si une liste blanche est définie
/// (disponibles immédiatement), sinon globalement.
pub async fn deploy(ctx: &Ctx) -> Result<()> {
    let commands = definitions(ctx.modmail.is_some());
    let client = ctx.interaction();
    if ctx.config.guild_whitelist.is_empty() {
        client.set_global_commands(&commands).await?;
        tracing::info!(count = commands.len(), "commandes déployées globalement (aucune liste blanche)");
    } else {
        for guild_id in &ctx.config.guild_whitelist {
            client.set_guild_commands(*guild_id, &commands).await?;
            tracing::info!(count = commands.len(), %guild_id, "commandes déployées sur la guilde");
        }
    }
    Ok(())
}

pub async fn dispatch(ctx: Context, interaction: Interaction) {
    let is_autocomplete = interaction.kind == InteractionType::ApplicationCommandAutocomplete;
    let ix = Ix::new(ctx, interaction);
    if let Err(error) = route(&ix).await {
        tracing::error!(error = ?error, "erreur lors du traitement d'une interaction");
        if !is_autocomplete {
            // L'interaction a pu expirer ou déjà recevoir une réponse : rien à faire.
            let _ = ix.notify("Une erreur est survenue.").await;
        }
    }
}

async fn route(ix: &Ix) -> Result<()> {
    let Some(data) = ix.interaction.data.as_ref() else {
        return Ok(());
    };

    match (ix.interaction.kind, data) {
        (InteractionType::ApplicationCommand, InteractionData::ApplicationCommand(command))
            if command.kind != CommandType::ChatInput =>
        {
            super::context_menus::handle(ix, command).await
        }
        (InteractionType::ApplicationCommand, InteractionData::ApplicationCommand(command)) => {
            let options = Options::parse(command);
            match command.name.as_str() {
                "presence" => presences::commands::handle_presence(ix, &options).await,
                "absence" => presences::commands::handle_absence(ix, &options).await,
                "transaction" => transactions::commands::handle(ix, &options).await,
                "vehicle" => vehicles::commands::handle(ix, &options).await,
                "recrue" => recrues::commands::handle(ix, &options).await,
                "numero" => phones::commands::handle(ix, &options).await,
                "cotisation" => crate::modules::cotisations::commands::handle(ix, &options).await,
                "config" => super::admin::handle_config(ix, &options).await,
                "gestion" => super::admin::handle_gestion(ix, &options).await,
                _ => Ok(()),
            }
        }
        (InteractionType::ApplicationCommandAutocomplete, InteractionData::ApplicationCommand(command)) => {
            let options = Options::parse(command);
            match command.name.as_str() {
                "presence" => presences::commands::autocomplete(ix, &options).await,
                "transaction" => transactions::commands::autocomplete(ix, &options).await,
                // `/gestion` reprend les options « presence » et « transaction » des modules.
                "gestion" => match options.group() {
                    Some("presence") if options.sub() == "deplanifier" => presences::planning::autocomplete(ix).await,
                    Some("presence") => presences::commands::autocomplete(ix, &options).await,
                    Some("transaction") => transactions::commands::autocomplete(ix, &options).await,
                    _ => ix.autocomplete(Vec::new()).await,
                },
                "vehicle" => vehicles::commands::autocomplete(ix, &options).await,
                "cotisation" => crate::modules::cotisations::commands::autocomplete(ix, &options).await,
                _ => ix.autocomplete(Vec::new()).await,
            }
        }
        (InteractionType::MessageComponent, InteractionData::MessageComponent(component)) => {
            let Some(custom_id) = decode_custom_id(&component.custom_id) else {
                return Ok(());
            };
            match custom_id.namespace {
                "presence" => presences::interactions::button(ix, &custom_id).await,
                "transaction" => transactions::interactions::button(ix, &custom_id).await,
                "numero" => phones::interactions::button(ix, &custom_id).await,
                presences::absence_panel::NAMESPACE => presences::absence_panel::button(ix, &custom_id).await,
                // « prospect » : messages publiés avant le renommage du module.
                "recrue" | "prospect" => recrues::interactions::select(ix, &custom_id, &component.values).await,
                crate::modmail::buttons::NAMESPACE => crate::modmail::buttons::button(ix, &custom_id).await,
                _ => Ok(()),
            }
        }
        (InteractionType::ModalSubmit, InteractionData::ModalSubmit(modal)) => {
            let Some(custom_id) = decode_custom_id(&modal.custom_id) else {
                return Ok(());
            };
            match (custom_id.namespace, custom_id.action) {
                ("presence", "retard-modal") => presences::interactions::retard_modal(ix, custom_id.id, modal).await,
                ("transaction", "additem-modal") => {
                    transactions::interactions::add_item_modal(ix, custom_id.id, modal).await
                }
                ("recrue" | "prospect", "vote-modal" | "veto-modal") => {
                    recrues::interactions::modal(ix, custom_id.action, custom_id.id, modal).await
                }
                ("numero", "add-modal") => phones::interactions::add_modal(ix, modal).await,
                (presences::absence_panel::NAMESPACE, "declare-modal") => {
                    presences::absence_panel::modal(ix, modal).await
                }
                (crate::modmail::buttons::NAMESPACE, "reply-modal") => {
                    crate::modmail::buttons::modal(ix, &custom_id, modal).await
                }
                _ => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

fn find_component<'a>(
    components: &'a [ModalInteractionComponent],
    custom_id: &str,
) -> Option<&'a ModalInteractionComponent> {
    components.iter().find_map(|component| match component {
        ModalInteractionComponent::Label(label) => find_component(std::slice::from_ref(&label.component), custom_id),
        ModalInteractionComponent::ActionRow(row) => find_component(&row.components, custom_id),
        ModalInteractionComponent::TextInput(input) if input.custom_id == custom_id => Some(component),
        ModalInteractionComponent::StringSelect(select) if select.custom_id == custom_id => Some(component),
        _ => None,
    })
}

/// Valeur d'un champ texte de modal (`""` si absent).
pub fn modal_text(modal: &ModalInteractionData, custom_id: &str) -> String {
    match find_component(&modal.components, custom_id) {
        Some(ModalInteractionComponent::TextInput(input)) => input.value.clone(),
        _ => String::new(),
    }
}

/// Valeurs choisies dans un menu déroulant de modal.
pub fn modal_select(modal: &ModalInteractionData, custom_id: &str) -> Vec<String> {
    match find_component(&modal.components, custom_id) {
        Some(ModalInteractionComponent::StringSelect(select)) => select.values.clone(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::definitions;
    use twilight_validate::command::command;

    /// Discord refuse tout le lot si une seule commande dépasse ses limites.
    #[test]
    fn command_definitions_are_valid() {
        for with_tickets in [false, true] {
            for definition in definitions(with_tickets) {
                command(&definition).unwrap_or_else(|error| panic!("{} : {error}", definition.name));
            }
        }
    }
}
