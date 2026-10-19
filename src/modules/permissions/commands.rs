//! `/config permission` : rôle membre et niveau de chaque action.

use super::{Level, Perm, service};
use crate::{
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{Ix, MANAGE_GUILD_ONLY, Reply, choice},
        options::Options,
    },
    util::format::{field, truncate_field},
};
use anyhow::Result;
use std::collections::HashMap;
use twilight_model::guild::Permissions;
use twilight_util::builder::command::{RoleBuilder, StringBuilder, SubCommandBuilder};

const COLOR: u32 = 0x5865f2;
const DEFAULT: &str = "defaut";

pub fn config_subcommand() -> SubCommandBuilder {
    let mut levels: Vec<(&str, &str)> = Level::ALL.iter().map(|l| (l.label(), l.as_str())).collect();
    levels.push(("Par défaut", DEFAULT));
    SubCommandBuilder::new("permission", "Permissions : rôle membre et niveau de chaque action (sans option : affiche)")
        .option(RoleBuilder::new("role_membre", "Rôle des membres du groupe"))
        .option(StringBuilder::new("action", "Action à régler").autocomplete(true))
        .option(StringBuilder::new("niveau", "Qui peut faire cette action").choices(levels))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let role = options.role("role_membre");
    let action = options.string("action");
    let level = options.string("niveau");
    let perm = match action.as_deref() {
        Some(raw) => match Perm::parse(raw) {
            Some(perm) => Some(perm),
            None => return ix.reply(Reply::ephemeral("Action inconnue : choisis-la dans la liste proposée.")).await,
        },
        None => None,
    };
    if level.is_some() && perm.is_none() {
        return ix.reply(Reply::ephemeral("Indique l'action dont tu changes le niveau.")).await;
    }
    if role.is_none() && level.is_none() {
        return show(ix, guild, perm).await;
    }
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }

    let mut done = Vec::new();
    if let Some(role) = role {
        service::set_member_role(&ix.ctx.db, guild, Some(&role.to_string())).await?;
        done.push(format!("Rôle des membres du groupe : <@&{role}>."));
    }
    if let (Some(perm), Some(level)) = (perm, level) {
        let level = Level::parse(&level);
        service::set_level(&ix.ctx.db, guild, perm, level).await?;
        let level = level.unwrap_or(perm.default_level());
        done.push(format!("« {} » (`{}`) : {}.", perm.label(), perm.key(), level.label()));
    }
    ix.reply(Reply::ephemeral(done.join("\n"))).await
}

async fn show(ix: &Ix, guild: &str, only: Option<Perm>) -> Result<()> {
    let roles = service::roles(&ix.ctx.db, guild).await?;
    let levels = service::levels(&ix.ctx.db, guild).await?;
    let role = |r: &Option<String>| r.as_ref().map_or_else(|| "*non configuré*".to_owned(), |r| format!("<@&{r}>"));
    let mut embed = EmbedBuilder::new().title("Permissions").color(COLOR).description(format!(
        "Membre : {} · Gradé : {} · Lead : {}\nLes niveaux s'emboîtent : un lead peut tout ce que peut un gradé. \
         « Gérer les messages » compte comme gradé, « Gérer le serveur » ouvre tout sauf les avis et vétos.",
        role(&roles.member),
        role(&roles.grade),
        role(&roles.lead),
    ));
    let line = |perm: Perm, levels: &HashMap<String, Level>| {
        let level = levels.get(perm.key()).copied();
        let marker = if level.is_some_and(|l| l != perm.default_level()) { " ✏️" } else { "" };
        format!("`{}` {} — **{}**{marker}", perm.key(), perm.label(), level.unwrap_or(perm.default_level()).label())
    };
    let mut modules: Vec<(&str, Vec<String>)> = Vec::new();
    for perm in Perm::ALL.iter().copied().filter(|p| only.is_none_or(|o| o == *p)) {
        match modules.last_mut() {
            Some((module, lines)) if *module == perm.module() => lines.push(line(perm, &levels)),
            _ => modules.push((perm.module(), vec![line(perm, &levels)])),
        }
    }
    for (module, lines) in modules {
        embed = embed.field(field(module, truncate_field(&lines.join("\n")), false));
    }
    let embed = embed.footer(footer("✏️ niveau modifié · /config permission action:… niveau:…")).build();
    ix.reply(Reply::embed(embed).hidden()).await
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let query = options.focused.as_ref().map(|(_, v)| v.to_lowercase()).unwrap_or_default();
    let choices = Perm::ALL
        .iter()
        .filter(|p| {
            query.is_empty()
                || p.key().contains(&query)
                || p.label().to_lowercase().contains(&query)
                || p.module().to_lowercase().contains(&query)
        })
        .take(25)
        .map(|p| choice(&format!("{} · {}", p.module(), p.label()), p.key()))
        .collect();
    ix.autocomplete(choices).await
}
