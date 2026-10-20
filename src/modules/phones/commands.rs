use super::{INVALID_FORMAT, format, is_valid, normalize, service};
use crate::modules::audit::{self, Action, Entry};
use crate::{
    discord::{
        embed::EmbedBuilder,
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply},
        options::Options,
    },
    modules::permissions::{self, Perm},
    util::format::field,
};
use anyhow::Result;
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
};
use twilight_util::builder::command::{ChannelBuilder, CommandBuilder, StringBuilder, SubCommandBuilder, UserBuilder};

pub fn definition() -> Command {
    CommandBuilder::new("numero", "Gestion de l'annuaire téléphonique", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("ajouter", "Enregistre ton numéro de téléphone")
                .option(StringBuilder::new("numero", "Ton numéro (ex: 5551234)").required(true))
                .option(UserBuilder::new(
                    "membre",
                    "Inscrire le numéro d'un autre membre (« Gérer le serveur » requis)",
                )),
        )
        .option(
            SubCommandBuilder::new("voir", "Consulte le numéro d'un membre")
                .option(UserBuilder::new("membre", "Le membre ciblé (laisser vide pour soi-même)")),
        )
        .option(SubCommandBuilder::new("supprimer", "Supprime un numéro de téléphone").option(UserBuilder::new(
            "membre",
            "Le membre ciblé (laisser vide pour soi-même, gradé requis pour les autres)",
        )))
        .option(SubCommandBuilder::new("liste", "Republie la liste des numéros"))
        .build()
}

/// `/config numero`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("numero", "Annuaire : salon d'affichage (sans option : affiche)")
        .option(ChannelBuilder::new("salon", "Le salon où publier l'annuaire").channel_types([ChannelType::GuildText]))
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    match options.sub() {
        "ajouter" => ajouter(ix, options, &guild).await,
        "voir" => voir(ix, options, &guild).await,
        "supprimer" => supprimer(ix, options, &guild).await,
        "liste" => {
            ix.defer(true).await?;
            service::sync_dashboard(&ix.ctx, &guild).await?;
            ix.edit_text("Annuaire republié dans le salon configuré.").await
        }
        _ => Ok(()),
    }
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    // Sans option, la commande sert à consulter la configuration (lecture ouverte à tous).
    let Some(channel) = options.channel("salon") else {
        let config = service::get_config(&ix.ctx.db, guild).await?;
        let embed = EmbedBuilder::new()
            .title("Configuration de l'annuaire")
            .color(0x5865f2)
            .field(field(
                "Salon de l'annuaire",
                config
                    .and_then(|c| c.dashboard_channel_id)
                    .map_or("*non configuré*".to_owned(), |id| format!("<#{id}>")),
                false,
            ))
            .build();
        return ix.reply(Reply::embed(embed).hidden()).await;
    };
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    ix.defer(true).await?;
    service::set_dashboard_channel(&ix.ctx.db, guild, &channel.to_string()).await?;
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text(format!("Annuaire installé dans <#{channel}>.")).await
}

async fn ajouter(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let numero = normalize(&options.string("numero").unwrap_or_default());
    if !is_valid(&numero) {
        return ix.reply(Reply::ephemeral(INVALID_FORMAT)).await;
    }

    // Inscrire le numéro de quelqu'un d'autre est une action d'administration :
    // elle écrase sans confirmation le numéro que le membre avait choisi.
    let membre = options.user("membre");
    let forced = membre.filter(|id| *id != ix.user_id());
    if forced.is_some() && !permissions::can(ix, Perm::NumeroAjouterAutrui).await? {
        return ix
            .reply(Reply::ephemeral("Tu n'as pas la permission d'enregistrer le numéro d'un autre membre."))
            .await;
    }

    let target = membre.unwrap_or(ix.user_id());
    if !permissions::require_member(ix, target, &options.user_name(target)).await? {
        return Ok(());
    }
    ix.defer(true).await?;
    if !service::set(&ix.ctx.db, guild, &target.to_string(), &numero).await? {
        return ix.edit_text(service::NUMBER_TAKEN).await;
    }
    service::sync_dashboard(&ix.ctx, guild).await?;
    if let Some(id) = forced.filter(|id| *id != ix.user_id()) {
        audit::record(ix, Entry::new(Action::PhoneSetFor, Some(id.to_string()), format(&numero))).await;
    }
    ix.edit_text(match forced {
        Some(id) => format!("Numéro `{}` enregistré pour {}.", format(&numero), options.user_name(id)),
        None => format!("Numéro enregistré : `{}`", format(&numero)),
    })
    .await
}

async fn voir(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let target = options.user("membre").unwrap_or(ix.user_id());
    let name = if target == ix.user_id() { ix.username() } else { options.user_name(target) };
    let found = service::get(&ix.ctx.db, guild, &target.to_string()).await?;
    ix.reply(Reply::ephemeral(match found {
        Some(entry) => format!("Numéro de {name} : `{}`", format(&entry.numero)),
        None => format!("{name} n'a pas de numéro enregistré."),
    }))
    .await
}

async fn supprimer(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let membre = options.user("membre");
    if membre.is_some_and(|id| id != ix.user_id()) && !permissions::require(ix, Perm::NumeroSupprimerAutrui).await? {
        return Ok(());
    }
    let target = membre.unwrap_or(ix.user_id());
    ix.defer(true).await?;
    if service::remove(&ix.ctx.db, guild, &target.to_string()).await? {
        service::sync_dashboard(&ix.ctx, guild).await?;
        if target != ix.user_id() {
            audit::record(ix, Entry::new(Action::PhoneRemovedFor, Some(target.to_string()), String::new())).await;
        }
        ix.edit_text("Numéro supprimé avec succès.").await
    } else {
        ix.edit_text("Aucun numéro enregistré pour ce membre.").await
    }
}
