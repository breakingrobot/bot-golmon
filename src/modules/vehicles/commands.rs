use super::{
    api::{Lookup, normalize_plate},
    embed::build_lookup,
    service,
};
use crate::{
    discord::{
        embed::EmbedBuilder,
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        options::{Mentionable, Options},
    },
    modules::permissions::{self, Perm},
    util::format::{field, owner_mention},
};
use anyhow::Result;
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
};
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, MentionableBuilder, StringBuilder, SubCommandBuilder,
};

fn registered_plate() -> StringBuilder {
    StringBuilder::new("plaque", "Plaque d'immatriculation").required(true).autocomplete(true)
}

pub fn definition() -> Command {
    CommandBuilder::new("vehicle", "Registre des véhicules du groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("add", "Enregistre un véhicule (le modèle est récupéré automatiquement)")
                .option(StringBuilder::new("plaque", "Plaque d'immatriculation").required(true))
                .option(MentionableBuilder::new("proprietaire", "Propriétaire Discord (Membre ou Rôle)").required(true))
                .option(StringBuilder::new("modele", "Modèle (sinon récupéré via l'API)")),
        )
        .option(
            SubCommandBuilder::new("lookup", "Informations d'un véhicule à partir de sa plaque")
                .option(StringBuilder::new("plaque", "Plaque d'immatriculation").required(true)),
        )
        .option(
            SubCommandBuilder::new("owner", "Change le propriétaire d'un véhicule enregistré")
                .option(registered_plate())
                .option(
                    MentionableBuilder::new("proprietaire", "Nouveau propriétaire (Membre ou Rôle)").required(true),
                ),
        )
        .option(SubCommandBuilder::new("remove", "Retire un véhicule du registre").option(registered_plate()))
        .build()
}

/// `/config vehicule`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("vehicule", "Véhicules : salon du tableau de bord (sans option : affiche)").option(
        ChannelBuilder::new("dashboard_channel", "Salon du tableau de bord des véhicules")
            .channel_types([ChannelType::GuildText]),
    )
}

fn owner(options: &Options) -> Option<(String, bool)> {
    match options.mentionable("proprietaire")? {
        Mentionable::Role(id) => Some((id.to_string(), true)),
        Mentionable::User(id) => Some((id.to_string(), false)),
    }
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    let query = options.focused.as_ref().map(|(_, v)| v.as_str()).unwrap_or_default();
    let vehicles = service::search(&ix.ctx.db, &guild_id.to_string(), query).await?;
    let choices = vehicles
        .iter()
        .map(|v| {
            choice(
                &format!("{}{}", v.plate, v.modele.as_deref().map(|m| format!(" — {m}")).unwrap_or_default()),
                v.plate.clone(),
            )
        })
        .collect();
    ix.autocomplete(choices).await
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    if !permissions::require(ix, Perm::Vehicule).await? {
        return Ok(());
    }
    if options.sub() == "lookup" {
        return lookup(ix, options).await;
    }
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    match options.sub() {
        "add" => add(ix, options, &guild).await,
        "owner" => change_owner(ix, options, &guild).await,
        "remove" => remove(ix, options, &guild).await,
        _ => Ok(()),
    }
}

fn already_registered(plate: &str) -> String {
    format!("La plaque `{plate}` est déjà enregistrée. Utilise `/vehicle owner` pour la transférer.")
}

async fn add(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let plate = normalize_plate(&options.string("plaque").unwrap_or_default());
    if plate.is_empty() {
        return ix.reply(Reply::ephemeral("Plaque invalide.")).await;
    }
    let Some((owner_id, owner_is_role)) = owner(options) else {
        return ix.reply(Reply::ephemeral("Propriétaire invalide.")).await;
    };
    if service::get_by_plate(&ix.ctx.db, guild, &plate).await?.is_some() {
        return ix.reply(Reply::ephemeral(already_registered(&plate))).await;
    }

    ix.defer(true).await?;

    // Pré-remplissage : une API indisponible ne doit jamais bloquer l'ajout.
    let mut modele = options.string("modele");
    let mut api_note = "";
    if modele.is_none() {
        match ix.ctx.vehicles.lookup(&plate).await {
            Lookup::Found(vehicle) => modele = vehicle.name.or(vehicle.model),
            Lookup::NotFound => api_note = " (plaque inconnue de l'API, modèle non renseigné)",
            Lookup::Error(_) => api_note = " (API injoignable, modèle non renseigné)",
        }
    }

    if !service::add(&ix.ctx.db, guild, &plate, &owner_id, owner_is_role, modele.as_deref()).await? {
        return ix.edit_text(already_registered(&plate)).await;
    }
    service::sync_dashboard(&ix.ctx, guild).await?;
    let modele = modele.map(|m| format!(" — {m}")).unwrap_or_default();
    ix.edit_text(format!("`{plate}`{modele} enregistré pour {}{api_note}.", owner_mention(&owner_id, owner_is_role)))
        .await
}

async fn remove(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let plate = normalize_plate(&options.string("plaque").unwrap_or_default());
    if service::get_by_plate(&ix.ctx.db, guild, &plate).await?.is_none() {
        return ix.reply(Reply::ephemeral(format!("Aucun véhicule enregistré avec la plaque `{plate}`."))).await;
    }
    ix.defer(true).await?;
    service::remove(&ix.ctx.db, guild, &plate).await?;
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text(format!("`{plate}` retiré du registre.")).await
}

async fn change_owner(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let plate = normalize_plate(&options.string("plaque").unwrap_or_default());
    let Some((owner_id, owner_is_role)) = owner(options) else {
        return ix.reply(Reply::ephemeral("Propriétaire invalide.")).await;
    };
    if service::get_by_plate(&ix.ctx.db, guild, &plate).await?.is_none() {
        return ix.reply(Reply::ephemeral(format!("Aucun véhicule enregistré avec la plaque `{plate}`."))).await;
    }
    ix.defer(true).await?;
    service::set_owner(&ix.ctx.db, guild, &plate, &owner_id, owner_is_role).await?;
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text(format!("`{plate}` appartient désormais à {}.", owner_mention(&owner_id, owner_is_role))).await
}

async fn lookup(ix: &Ix, options: &Options) -> Result<()> {
    let plate = normalize_plate(&options.string("plaque").unwrap_or_default());
    if plate.is_empty() {
        return ix.reply(Reply::ephemeral("Plaque invalide.")).await;
    }
    ix.defer(true).await?;
    match ix.ctx.vehicles.lookup(&plate).await {
        Lookup::NotFound => ix.edit_text(format!("Aucun véhicule trouvé pour la plaque `{plate}`.")).await,
        Lookup::Error(reason) => {
            ix.edit_text(format!("Impossible d'interroger l'API pour le moment ({reason}). Réessaie plus tard.")).await
        }
        Lookup::Found(vehicle) => {
            let registered = match ix.guild_id() {
                Some(guild_id) => service::get_by_plate(&ix.ctx.db, &guild_id.to_string(), &plate).await?,
                None => None,
            };
            ix.edit(Reply::embed(build_lookup(&vehicle, registered.as_ref()))).await
        }
    }
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let Some(channel) = options.channel("dashboard_channel") else {
        let config = service::get_config(&ix.ctx.db, guild).await?;
        let embed = EmbedBuilder::new()
            .title("Configuration des véhicules")
            .color(0x5865f2)
            .field(field(
                "Salon du tableau de bord",
                config
                    .and_then(|c| c.dashboard_channel_id)
                    .map_or("*non configuré*".to_owned(), |id| format!("<#{id}>")),
                false,
            ))
            .build();
        return ix.reply(Reply::embed(embed).hidden()).await;
    };
    ix.defer(true).await?;
    service::set_dashboard_channel(&ix.ctx.db, guild, &channel.to_string()).await?;
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text(format!("Tableau de bord installé dans <#{channel}>.")).await
}
