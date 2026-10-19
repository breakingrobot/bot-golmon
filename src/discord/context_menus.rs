//! Actions au clic droit (menu « Applications ») sur un membre ou un message.

use super::{
    interaction::{GUILD_ONLY, Ix, Reply},
    options::Options,
};
use crate::modules::{
    permissions::{self, Perm},
    presences, recrues,
};
use anyhow::Result;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::{InteractionContextType, application_command::CommandData},
    },
    guild::Permissions,
};
use twilight_util::builder::command::CommandBuilder;

pub const MEMBER_PRESENCES: &str = "Présences du membre";
pub const ADD_RECRUE: &str = "Ajouter aux recrues";
pub const OPEN_TICKET: &str = "Ouvrir un ticket";
pub const REPORT: &str = "Signaler au staff";

/// Les actions liées aux tickets n'existent que si le module est configuré.
pub fn definitions(with_tickets: bool) -> Vec<Command> {
    let menu =
        |name: &str, kind: CommandType| CommandBuilder::new(name, "", kind).contexts([InteractionContextType::Guild]);
    let mut commands = vec![
        menu(MEMBER_PRESENCES, CommandType::User).build(),
        // Visible de tous ; réservé au niveau de `recrue.suivi` (`/config permission`).
        menu(ADD_RECRUE, CommandType::User).build(),
    ];
    if with_tickets {
        commands.push(
            menu(OPEN_TICKET, CommandType::User).default_member_permissions(Permissions::MANAGE_MESSAGES).build(),
        );
        commands.push(menu(REPORT, CommandType::Message).build());
    }
    commands
}

pub async fn handle(ix: &Ix, command: &CommandData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let Some(target) = command.target_id else {
        return Ok(());
    };
    let options = Options::parse(command);
    let resolved = command.resolved.as_ref();
    match command.name.as_str() {
        MEMBER_PRESENCES => {
            if !permissions::require(ix, Perm::PresenceStats).await? {
                return Ok(());
            }
            presences::commands::member_stats(ix, &options, target.cast()).await
        }
        ADD_RECRUE => recrues::commands::add_member(ix, &options, guild_id, target.cast()).await,
        OPEN_TICKET => {
            let (Some(modmail), Some(user)) =
                (ix.ctx.modmail.as_ref(), resolved.and_then(|r| r.users.get(&target.cast())))
            else {
                return Ok(());
            };
            modmail.open_ticket_for(ix, user).await
        }
        REPORT => {
            let (Some(modmail), Some(message)) =
                (ix.ctx.modmail.as_ref(), resolved.and_then(|r| r.messages.get(&target.cast())))
            else {
                return Ok(());
            };
            modmail.report_message(ix, message).await
        }
        _ => Ok(()),
    }
}
