//! Commandes d'administration, masquées par défaut aux membres :
//! `/config` (« Gérer le serveur ») et `/gestion` (« Gérer les messages »).
//!
//! Discord n'applique une permission par défaut qu'à une commande entière : les
//! réglages et actions de staff de chaque module sont donc regroupés ici, et les
//! commandes des modules ne gardent que ce qui sert à tout le monde. Les admins
//! du serveur peuvent ajuster la visibilité dans *Paramètres → Intégrations*.
//! Les permissions restent vérifiées à l'exécution.

use super::{
    interaction::{GUILD_ONLY, Ix, Reply},
    options::Options,
};
use crate::modules::{phones, presences, recrues, transactions, vehicles};
use anyhow::Result;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::InteractionContextType,
    },
    guild::Permissions,
};
use twilight_util::builder::command::{CommandBuilder, SubCommandGroupBuilder};

pub fn config_definition() -> Command {
    CommandBuilder::new("config", "Configuration des modules du bot", CommandType::ChatInput)
        .default_member_permissions(Permissions::MANAGE_GUILD)
        .contexts([InteractionContextType::Guild])
        .option(presences::commands::config_subcommand())
        .option(transactions::commands::config_subcommand())
        .option(vehicles::commands::config_subcommand())
        .option(recrues::commands::config_subcommand())
        .option(phones::commands::config_subcommand())
        .build()
}

pub fn gestion_definition() -> Command {
    CommandBuilder::new("gestion", "Actions réservées au staff", CommandType::ChatInput)
        .default_member_permissions(Permissions::MANAGE_MESSAGES)
        .contexts([InteractionContextType::Guild])
        .option(
            SubCommandGroupBuilder::new("presence", "Présences : votes forcés, annulation, planning")
                .subcommands(presences::commands::admin_subcommands()),
        )
        .option(
            SubCommandGroupBuilder::new("blocklist", "Membres jamais pingés, absents d'office")
                .subcommands(presences::commands::blocklist_subcommands()),
        )
        .option(
            SubCommandGroupBuilder::new("absence", "Absences : panneau de déclaration")
                .subcommands(presences::absence_panel::admin_subcommands()),
        )
        .option(
            SubCommandGroupBuilder::new("transaction", "Transactions")
                .subcommands(transactions::commands::admin_subcommands()),
        )
        .option(SubCommandGroupBuilder::new("recrue", "Recrues").subcommands(recrues::commands::admin_subcommands()))
        .build()
}

pub async fn handle_config(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    match options.sub() {
        "presence" => presences::commands::config(ix, options).await,
        "transaction" => transactions::commands::config(ix, options, &guild).await,
        "vehicule" => vehicles::commands::config(ix, options, &guild).await,
        "recrue" => recrues::commands::config(ix, options, guild_id).await,
        "numero" => phones::commands::config(ix, options, &guild).await,
        _ => Ok(()),
    }
}

pub async fn handle_gestion(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    match options.group() {
        Some("presence" | "blocklist") => presences::commands::handle_admin(ix, options).await,
        Some("absence") => presences::absence_panel::handle_admin(ix, options).await,
        Some("transaction") => transactions::commands::delete(ix, options, &guild_id.to_string()).await,
        Some("recrue") => recrues::commands::reset(ix, guild_id).await,
        _ => Ok(()),
    }
}
