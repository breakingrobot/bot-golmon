//! `/rappels` (chacun ses MP) et `/config rappels` (« Gérer le serveur »).

use super::{Config, MAX_PRESENCE_HOURS, MemberPrefs, OBJECTIVE_HOUR, service};
use crate::{
    discord::{
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply},
        options::Options,
    },
    modules::permissions,
};
use anyhow::Result;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::InteractionContextType,
    },
    guild::Permissions,
};
use twilight_util::builder::command::{BooleanBuilder, CommandBuilder, IntegerBuilder, SubCommandBuilder};

pub fn definition() -> Command {
    CommandBuilder::new(
        "rappels",
        "Tes rappels en message privé (sans option : voir où tu en es)",
        CommandType::ChatInput,
    )
    .contexts([InteractionContextType::Guild])
    .option(BooleanBuilder::new("presences", "MP avant une présence à laquelle tu n'as pas répondu"))
    .option(BooleanBuilder::new("objectif", "MP le samedi si tu n'as pas atteint l'objectif de cotisation"))
    .option(BooleanBuilder::new("tout", "false : couper tous les rappels · true : tout réactiver"))
    .build()
}

/// `/config rappels`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("rappels", "Rappels personnels en MP, désactivés par défaut (sans option : affiche)")
        .option(BooleanBuilder::new("actif", "Envoyer les rappels en MP sur ce serveur"))
        .option(
            IntegerBuilder::new(
                "presence_heures",
                "MP aux non-répondants quand la présence commence dans moins de N h",
            )
            .min_value(1)
            .max_value(MAX_PRESENCE_HOURS),
        )
        .option(BooleanBuilder::new("objectif", "MP le samedi 18:00 aux membres sous l'objectif de cotisation"))
}

fn on_off(on: bool) -> &'static str {
    if on { "✅ reçus" } else { "🔕 coupés" }
}

/// État des rappels d'un membre, avec les commandes pour changer.
pub fn state_text(config: &Config, prefs: &MemberPrefs) -> String {
    if !config.enabled {
        return "Les rappels en message privé ne sont pas activés sur ce serveur (un admin : \
                `/config rappels actif:true`)."
            .to_owned();
    }
    let objective = if config.objective {
        format!("{} (samedi {OBJECTIVE_HOUR}:00 si tu es en dessous)", on_off(!prefs.objective_off))
    } else {
        "non activé sur ce serveur".to_owned()
    };
    let mut text = format!(
        "🔔 **Tes rappels en message privé**\n• Présence sans réponse ({} h avant) : {}\n• Objectif de cotisation : {objective}",
        config.presence_hours,
        on_off(!prefs.presence_off),
    );
    if prefs.dm_closed_at.is_some() {
        text.push_str(
            "\n⚠️ Ton dernier rappel n'a pas pu partir : tes messages privés semblent fermés pour ce serveur.",
        );
    }
    text.push_str("\n-# Changer : `/rappels presences:false`, `/rappels objectif:false`, `/rappels tout:false`.");
    text
}

/// Applique les options de `/rappels` aux choix actuels (`tout` d'abord, puis le détail).
pub fn apply(
    mut prefs: MemberPrefs,
    all: Option<bool>,
    presences: Option<bool>,
    objective: Option<bool>,
) -> MemberPrefs {
    if let Some(all) = all {
        prefs.presence_off = !all;
        prefs.objective_off = !all;
    }
    if let Some(on) = presences {
        prefs.presence_off = !on;
    }
    if let Some(on) = objective {
        prefs.objective_off = !on;
    }
    prefs
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    // Rien n'est enregistré au nom de quelqu'un hors du groupe.
    if !permissions::require_member(ix, ix.user_id(), &ix.username()).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let user = ix.user_id().to_string();
    let config = service::get_config(&ix.ctx.db, &guild).await?;
    let mut prefs = service::member_prefs(&ix.ctx.db, &guild, &user).await?;
    let (all, presences, objective) =
        (options.boolean("tout"), options.boolean("presences"), options.boolean("objectif"));
    if all.is_some() || presences.is_some() || objective.is_some() {
        prefs = apply(prefs, all, presences, objective);
        service::set_prefs(&ix.ctx.db, &guild, &user, &prefs).await?;
        prefs.dm_closed_at = None;
    }
    ix.reply(Reply::ephemeral(state_text(&config, &prefs))).await
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let mut config = service::get_config(&ix.ctx.db, guild).await?;
    let show_only = options.summary().is_empty();
    if let Some(enabled) = options.boolean("actif") {
        config.enabled = enabled;
    }
    if let Some(hours) = options.integer("presence_heures") {
        config.presence_hours = hours.clamp(1, MAX_PRESENCE_HOURS);
    }
    if let Some(objective) = options.boolean("objectif") {
        config.objective = objective;
    }
    if !show_only {
        service::set_config(&ix.ctx.db, guild, &config).await?;
    }
    ix.reply(Reply::ephemeral(config_text(&config))).await
}

/// Réglages du serveur, pour `/config rappels`.
pub fn config_text(config: &Config) -> String {
    if !config.enabled {
        return format!(
            "Rappels en MP **désactivés**. `/config rappels actif:true` les active (présence {} h avant, objectif {}).",
            config.presence_hours,
            if config.objective { "le samedi" } else { "désactivé" }
        );
    }
    format!(
        "Rappels en MP **activés** :\n• présence sans réponse : MP quand elle commence dans moins de **{} h**\n\
         • objectif de cotisation : {}\nChaque membre peut couper les siens avec `/rappels`.",
        config.presence_hours,
        if config.objective {
            format!("MP le samedi à {OBJECTIVE_HOUR}:00 aux membres en dessous")
        } else {
            "désactivé (`/config rappels objectif:true`)".to_owned()
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_choices() {
        let base = MemberPrefs::default();
        let off = apply(base, Some(false), None, None);
        assert!(off.presence_off && off.objective_off);
        let only_objective = apply(base, Some(false), None, Some(true));
        assert!(only_objective.presence_off && !only_objective.objective_off);
        assert_eq!(apply(off, Some(true), None, None), base);
        assert!(apply(base, None, Some(false), None).presence_off);
    }

    #[test]
    fn explains_state() {
        let config = Config { enabled: true, presence_hours: 2, objective: true };
        let text = state_text(&config, &MemberPrefs { presence_off: true, ..Default::default() });
        assert!(text.contains("Présence sans réponse (2 h avant) : 🔕 coupés"));
        assert!(text.contains("Objectif de cotisation : ✅ reçus (samedi 18:00"));
        assert!(state_text(&Config::default(), &MemberPrefs::default()).contains("`/config rappels actif:true`"));
        assert!(config_text(&config).contains("moins de **2 h**"));
        assert!(config_text(&Config::default()).contains("**désactivés**"));
    }

    #[test]
    fn slash_command_is_valid() {
        twilight_validate::command::command(&definition()).unwrap();
    }
}
