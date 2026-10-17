use super::{
    embed::build_embed,
    model::{Status, TYPE_CHOICES},
    service::{self, NewTransaction},
};
use crate::{
    discord::{
        embed::EmbedBuilder,
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        messages::{self, parse_id},
        options::Options,
    },
    util::format::{field, format_price},
};
use anyhow::Result;
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, IntegerBuilder, NumberBuilder, RoleBuilder, StringBuilder,
    SubCommandBuilder,
};

pub const CLOSED: &str = "Cette transaction est clôturée et ne peut plus être modifiée.";

fn transaction_option() -> StringBuilder {
    StringBuilder::new("transaction", "Transaction concernée").required(true).autocomplete(true)
}

pub fn definition() -> Command {
    CommandBuilder::new("transaction", "Gestion des transactions du groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("create", "Crée une transaction")
                .option(StringBuilder::new("groupe", "Groupe concerné").required(true))
                .option(StringBuilder::new("type", "Achat ou vente").required(true).choices(TYPE_CHOICES))
                .option(NumberBuilder::new("prix", "Prix total").required(true)),
        )
        .option(
            SubCommandBuilder::new("add-item", "Ajoute un objet à une transaction")
                .option(transaction_option())
                .option(StringBuilder::new("nom", "Nom de l'objet").required(true))
                .option(IntegerBuilder::new("quantite", "Quantité (défaut: 1)").min_value(1))
                .option(NumberBuilder::new("prix_unitaire", "Prix unitaire")),
        )
        .option(
            SubCommandBuilder::new("status", "Change le statut d'une transaction").option(transaction_option()).option(
                StringBuilder::new("statut", "Nouveau statut")
                    .required(true)
                    .choices(Status::CHOICES.map(|s| (s.label(), s.as_str()))),
            ),
        )
        .option(SubCommandBuilder::new("view", "Affiche une transaction").option(transaction_option()))
        .option(SubCommandBuilder::new("cancel", "Annule une transaction").option(transaction_option()))
        .build()
}

/// `/config transaction`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("transaction", "Transactions : tableau de bord et rôle mentionné (sans option : affiche)")
        .option(
            ChannelBuilder::new("dashboard_channel", "Salon du tableau de bord permanent")
                .channel_types([ChannelType::GuildText]),
        )
        .option(RoleBuilder::new("ping_role", "Rôle à mentionner lors de la création d'une transaction"))
        .option(BooleanBuilder::new("clear_ping_role", "Ne plus mentionner aucun rôle à la création"))
}

/// `/gestion transaction supprimer`.
pub fn admin_subcommands() -> Vec<SubCommandBuilder> {
    vec![
        SubCommandBuilder::new("supprimer", "Supprime définitivement une transaction de la base de données")
            .option(transaction_option()),
    ]
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    // `view` doit pouvoir retrouver aussi les transactions terminées.
    let open_only = options.sub() != "view";
    let query = options.focused.as_ref().map(|(_, v)| v.as_str()).unwrap_or_default();
    let transactions = service::search(&ix.ctx.db, &guild_id.to_string(), query, open_only).await?;
    let choices = transactions
        .iter()
        .map(|t| choice(&format!("{} · {} · {}", t.groupe, format_price(t.prix), t.status().label()), t.id.clone()))
        .collect();
    ix.autocomplete(choices).await
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    match options.sub() {
        "create" => create(ix, options, &guild).await,
        "add-item" => add_item(ix, options, &guild).await,
        "status" => status(ix, options, &guild).await,
        "view" => view(ix, options, &guild).await,
        "cancel" => cancel(ix, options, &guild).await,
        _ => Ok(()),
    }
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let dashboard_channel = options.channel("dashboard_channel");
    let ping_role = options.role("ping_role");
    let clear_ping_role = options.boolean("clear_ping_role").unwrap_or(false);

    // Aucune option fournie : on affiche la configuration courante.
    if dashboard_channel.is_none() && ping_role.is_none() && !clear_ping_role {
        let config = service::get_config(&ix.ctx.db, guild).await?;
        let embed = EmbedBuilder::new()
            .title("Configuration des transactions")
            .color(0x5865f2)
            .field(field(
                "Salon du tableau de bord",
                config
                    .as_ref()
                    .and_then(|c| c.dashboard_channel_id.as_ref())
                    .map_or("*non configuré*".to_owned(), |id| format!("<#{id}>")),
                false,
            ))
            .field(field(
                "Rôle à ping",
                config
                    .as_ref()
                    .and_then(|c| c.ping_role_id.as_ref())
                    .map_or("*aucun*".to_owned(), |id| format!("<@&{id}>")),
                false,
            ))
            .build();
        return ix.reply(Reply::embed(embed).hidden()).await;
    }

    ix.defer(true).await?;
    let mut parts = Vec::new();
    if let Some(channel) = dashboard_channel {
        service::set_dashboard_channel(&ix.ctx.db, guild, &channel.to_string()).await?;
        parts.push(format!("Tableau de bord installé dans <#{channel}>."));
    }
    if clear_ping_role {
        service::set_ping_role(&ix.ctx.db, guild, None).await?;
        parts.push("Rôle à ping retiré.".to_owned());
    } else if let Some(role) = ping_role {
        service::set_ping_role(&ix.ctx.db, guild, Some(&role.to_string())).await?;
        parts.push(format!("Rôle à ping : <@&{role}>."));
    }
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text(parts.join(" ")).await
}

async fn create(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let Some(channel_id) = ix.channel_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    ix.defer(true).await?;
    let groupe = options.string("groupe").unwrap_or_default();
    // L'embed détaillé reste dans le salon de travail ; le tableau de bord donne la vue d'ensemble.
    let id = service::create(
        &ix.ctx.db,
        NewTransaction {
            guild_id: guild.to_owned(),
            channel_id: channel_id.to_string(),
            groupe: groupe.clone(),
            kind: options.string("type").filter(|t| t == "ACHAT").unwrap_or_else(|| "VENTE".to_owned()),
            prix: options.number("prix").unwrap_or_default(),
            created_by_id: ix.user_id().to_string(),
        },
    )
    .await?;
    // C'est cette publication qui mentionne le rôle configuré, le cas échéant.
    service::refresh(&ix.ctx, guild, &id).await?;
    ix.edit_text(format!("Transaction pour **{groupe}** créée avec succès.")).await
}

async fn add_item(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let id = options.string("transaction").unwrap_or_default();
    let nom = options.string("nom").unwrap_or_default();
    ix.defer(true).await?;

    let Some(transaction) = service::get(&ix.ctx.db, guild, &id).await? else {
        return ix.edit_text("Transaction introuvable. Assure-toi de la sélectionner correctement.").await;
    };
    if transaction.status() == Status::Cloturee {
        return ix.edit_text(CLOSED).await;
    }
    service::add_item(&ix.ctx.db, &id, &nom, options.integer("quantite").unwrap_or(1), options.number("prix_unitaire"))
        .await?;
    service::refresh(&ix.ctx, guild, &id).await?;
    ix.edit_text(format!("Objet « {nom} » ajouté.")).await
}

/// Volontairement ouvert à tous les membres du serveur : faire avancer une
/// transaction est une action courante du groupe, pas une action de modération.
/// Seule la suppression définitive est protégée.
async fn status(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let id = options.string("transaction").unwrap_or_default();
    let Some(status) = options.string("statut").as_deref().and_then(Status::parse) else {
        return ix.reply(Reply::ephemeral("Statut inconnu.")).await;
    };
    ix.defer(true).await?;

    let Some(transaction) = service::get(&ix.ctx.db, guild, &id).await? else {
        return ix.edit_text("Transaction introuvable.").await;
    };
    if transaction.status() == Status::Cloturee {
        return ix.edit_text(CLOSED).await;
    }
    service::set_status(&ix.ctx.db, guild, &id, status).await?;
    service::refresh(&ix.ctx, guild, &id).await?;
    ix.edit_text(format!("Statut passé à **{}**.", status.label())).await
}

async fn cancel(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let id = options.string("transaction").unwrap_or_default();
    ix.defer(true).await?;
    let Some(transaction) = service::get(&ix.ctx.db, guild, &id).await? else {
        return ix.edit_text("Transaction introuvable.").await;
    };
    if transaction.status() == Status::Cloturee {
        return ix.edit_text(CLOSED).await;
    }
    service::set_status(&ix.ctx.db, guild, &id, Status::Annulee).await?;
    service::refresh(&ix.ctx, guild, &id).await?;
    ix.edit_text("Transaction annulée.").await
}

pub async fn delete(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    // Seule action irréversible du module : elle reste réservée aux modérateurs.
    if !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix
            .reply(Reply::ephemeral(
                "Tu dois avoir la permission de gérer les messages pour supprimer une transaction.",
            ))
            .await;
    }
    let id = options.string("transaction").unwrap_or_default();
    ix.defer(true).await?;
    let Some(transaction) = service::get(&ix.ctx.db, guild, &id).await? else {
        return ix.edit_text("Transaction introuvable.").await;
    };
    if let (Some(channel_id), Some(message_id)) =
        (parse_id(&transaction.channel_id), transaction.message_id.as_deref().and_then(parse_id))
    {
        messages::delete_quietly(&ix.ctx, channel_id, message_id).await;
    }
    service::delete(&ix.ctx.db, guild, &id).await?;
    service::sync_dashboard(&ix.ctx, guild).await?;
    ix.edit_text("Transaction supprimée définitivement de la base de données.").await
}

async fn view(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let id = options.string("transaction").unwrap_or_default();
    match service::get(&ix.ctx.db, guild, &id).await? {
        Some(transaction) => ix.reply(Reply::embed(build_embed(&transaction)).hidden()).await,
        None => ix.reply(Reply::ephemeral("Transaction introuvable.")).await,
    }
}
