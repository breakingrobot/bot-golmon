use super::{
    ingest,
    parser::{Kind, item_label},
    service::{self, Filter, StockLine},
};
use crate::{
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        options::Options,
    },
    modules::{
        audit::{self, Action, Entry},
        permissions::{self, Perm},
    },
    util::{
        format::{DESCRIPTION_LIMIT, truncate},
        time::{DbTime, format_date_label},
    },
};
use anyhow::Result;
use chrono::Duration;
use std::collections::{BTreeMap, HashMap};
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
    id::{Id, marker::GuildMarker},
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, IntegerBuilder, StringBuilder, SubCommandBuilder, UserBuilder,
};

const COLOR: u32 = 0xe67e22;

fn property_option(required: bool) -> StringBuilder {
    StringBuilder::new("propriete", "Propriété").required(required).autocomplete(true)
}

pub fn definition() -> Command {
    CommandBuilder::new("coffre", "Coffres des propriétés (lus dans les messages du jeu)", CommandType::ChatInput)
        .option(SubCommandBuilder::new("stock", "Stock d'un coffre").option(property_option(false)))
        .option(
            SubCommandBuilder::new("historique", "Derniers dépôts et retraits")
                .option(property_option(false))
                .option(UserBuilder::new("membre", "Mouvements d'un membre (personnages reliés)"))
                .option(StringBuilder::new("objet", "Un seul objet").autocomplete(true))
                .option(IntegerBuilder::new("jours", "Les N derniers jours").min_value(1).max_value(3650)),
        )
        .option(
            SubCommandBuilder::new("lier", "Relie un personnage du jeu à un membre")
                .option(StringBuilder::new("id_jeu", "Identifiant du personnage, ex : 340772").required(true))
                .option(UserBuilder::new("membre", "Membre Discord").required(true)),
        )
        .option(
            SubCommandBuilder::new("nommer", "Donne un nom lisible à une propriété")
                .option(property_option(true))
                .option(StringBuilder::new("nom", "Nom affiché, ex : Planque du port").required(true).max_length(80)),
        )
        .build()
}

/// `/config coffre`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("coffre", "Coffres : salon du webhook du jeu (sans option : affiche)")
        .option(
            ChannelBuilder::new("salon", "Salon où le jeu publie les mouvements des coffres")
                .channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("retirer", "Ne plus suivre ce salon"))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let Some(channel_id) = options.channel("salon") else {
        let channels = service::channels(&ix.ctx.db, guild).await?;
        let text = if channels.is_empty() {
            "Aucun salon suivi. Ajoute celui du webhook avec `/config coffre salon:#…`.".to_owned()
        } else {
            format!(
                "Salons suivis : {}.",
                channels.iter().map(|(id, _)| format!("<#{id}>")).collect::<Vec<_>>().join(", ")
            )
        };
        return ix.reply(Reply::ephemeral(text)).await;
    };
    if options.boolean("retirer") == Some(true) {
        let removed = service::remove_channel(&ix.ctx.db, guild, &channel_id.to_string()).await?;
        return ix
            .reply(Reply::ephemeral(if removed {
                format!("<#{channel_id}> n'est plus suivi. Les mouvements déjà lus sont conservés.")
            } else {
                format!("<#{channel_id}> n'était pas suivi.")
            }))
            .await;
    }
    if !ix.ctx.message_content {
        return ix
            .reply(Reply::ephemeral(
                "Le bot ne peut pas lire les messages du webhook : active *Message Content Intent* dans le portail \
                 développeur Discord (onglet Bot), puis redémarre le bot.",
            ))
            .await;
    }
    service::add_channel(&ix.ctx.db, guild, &channel_id.to_string()).await?;
    ix.defer(true).await?;
    // Première lecture : l'historique récent du salon (jusqu'à 5 000 messages).
    let added = ingest::catch_up(&ix.ctx, guild, channel_id, None).await?;
    ix.edit_text(format!(
        "<#{channel_id}> est suivi : chaque dépôt et retrait publié par le jeu est enregistré. \
         {added} mouvement(s) importé(s) depuis l'historique du salon."
    ))
    .await
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let perm = if matches!(options.sub(), "lier" | "nommer") { Perm::CoffreGerer } else { Perm::CoffreVoir };
    if !permissions::require(ix, perm).await? {
        return Ok(());
    }
    match options.sub() {
        "stock" => stock(ix, options, guild_id).await,
        "historique" => history(ix, options, guild_id).await,
        "lier" => link(ix, options, guild_id).await,
        "nommer" => rename(ix, options, guild_id).await,
        _ => Ok(()),
    }
}

/// Nom affiché d'une propriété : nom donné, sinon « Propriété 8740 ».
pub fn property_label(names: &HashMap<String, String>, id: &str) -> String {
    names.get(id).cloned().unwrap_or_else(|| format!("Propriété {id}"))
}

async fn property_names(ix: &Ix, guild: &str) -> Result<HashMap<String, String>> {
    Ok(service::properties(&ix.ctx.db, guild)
        .await?
        .into_iter()
        .filter_map(|(id, name)| name.map(|name| (id, name)))
        .collect())
}

/// Lignes de stock groupées par propriété, objets épuisés masqués.
pub fn group_stock(lines: Vec<StockLine>) -> BTreeMap<String, Vec<StockLine>> {
    let mut grouped: BTreeMap<String, Vec<StockLine>> = BTreeMap::new();
    for line in lines.into_iter().filter(|l| l.quantity() != 0) {
        grouped.entry(line.property_id.clone()).or_default().push(line);
    }
    grouped
}

async fn stock(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let property = options.string("propriete");
    let names = property_names(ix, &guild).await?;
    let grouped = group_stock(service::stock(&ix.ctx.db, &guild, property.as_deref()).await?);
    let description = if grouped.is_empty() {
        "Aucun objet en stock d'après les mouvements enregistrés.".to_owned()
    } else {
        truncate(
            &grouped
                .iter()
                .map(|(property, lines)| {
                    let items = lines
                        .iter()
                        .map(|l| format!("• {} × **{}**", item_label(&l.item), l.quantity()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("### {}\n{items}", property_label(&names, property))
                })
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let embed = EmbedBuilder::new()
        .title("📦 Stock des coffres")
        .color(COLOR)
        .description(description)
        .footer(footer("Calculé à partir des dépôts et retraits lus depuis le début du suivi"))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn history(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let player_game_ids = match options.user("membre") {
        Some(user) => Some(service::game_ids_of(&ix.ctx.db, &guild, &user.to_string()).await?),
        None => None,
    };
    if player_game_ids.as_ref().is_some_and(Vec::is_empty) {
        return ix
            .reply(Reply::ephemeral(
                "Aucun personnage relié à ce membre. Relie-le avec `/coffre lier id_jeu:… membre:@…`.",
            ))
            .await;
    }
    let filter = Filter {
        property_id: options.string("propriete"),
        item: options.string("objet"),
        player_game_ids,
        kind: None,
        since: options.integer("jours").map(|d| DbTime(DbTime::now().0 - Duration::days(d))),
    };
    let names = property_names(ix, &guild).await?;
    let rows = service::movements(&ix.ctx.db, &guild, &filter, 20, 0).await?;
    let total = service::count(&ix.ctx.db, &guild, &filter).await?;
    let description = if rows.is_empty() {
        "Aucun mouvement.".to_owned()
    } else {
        truncate(
            &rows
                .iter()
                .map(|r| {
                    let sign = if r.kind() == Kind::Deposit { "📥" } else { "📤" };
                    format!(
                        "{sign} `{}` **{}** × {} — {} · {}",
                        format_date_label(r.at),
                        item_label(&r.item),
                        r.amount,
                        r.player_name,
                        property_label(&names, &r.property_id)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let mut footer_text = format!("{} mouvement(s) au total", total);
    if let Some(url) = crate::web::auth::public_url(&ix.ctx) {
        footer_text.push_str(&format!(" · détail : {url}/stats/{guild}/coffres"));
    }
    let embed = EmbedBuilder::new()
        .title("📦 Mouvements des coffres")
        .color(COLOR)
        .description(description)
        .footer(footer(footer_text))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn link(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let game_id = options.string("id_jeu").unwrap_or_default().trim().to_owned();
    if game_id.is_empty() || !game_id.bytes().all(|b| b.is_ascii_digit()) {
        return ix.reply(Reply::ephemeral("Identifiant invalide : uniquement des chiffres, ex : `340772`.")).await;
    }
    let member = options.user("membre").expect("option requise");
    service::link_player(&ix.ctx.db, &guild_id.to_string(), &game_id, &member.to_string()).await?;
    audit::record(ix, Entry::new(Action::CoffreLink, Some(member.to_string()), format!("Personnage `{game_id}`")))
        .await;
    ix.reply(Reply::ephemeral(format!("Le personnage `{game_id}` est relié à <@{member}>."))).await
}

async fn rename(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let property = options.string("propriete").unwrap_or_default().trim().to_owned();
    let name = options.string("nom").unwrap_or_default().trim().to_owned();
    if property.is_empty() || name.is_empty() {
        return ix.reply(Reply::ephemeral("Propriété et nom requis.")).await;
    }
    service::name_property(&ix.ctx.db, &guild_id.to_string(), &property, &name).await?;
    ix.reply(Reply::ephemeral(format!("La propriété `{property}` s'appelle désormais **{name}**."))).await
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    if !permissions::can(ix, Perm::CoffreVoir).await? {
        return ix.autocomplete(Vec::new()).await;
    }
    let guild = guild_id.to_string();
    let Some((name, typed)) = options.focused.clone() else {
        return ix.autocomplete(Vec::new()).await;
    };
    let typed = typed.to_lowercase();
    let choices = if name == "objet" {
        service::items(&ix.ctx.db, &guild)
            .await?
            .into_iter()
            .filter(|item| {
                typed.is_empty()
                    || item.to_lowercase().contains(&typed)
                    || item_label(item).to_lowercase().contains(&typed)
            })
            .take(25)
            .map(|item| choice(&item_label(&item), item))
            .collect()
    } else {
        service::properties(&ix.ctx.db, &guild)
            .await?
            .into_iter()
            .map(|(id, name)| (name.map_or_else(|| format!("Propriété {id}"), |n| format!("{n} ({id})")), id))
            .filter(|(label, _)| typed.is_empty() || label.to_lowercase().contains(&typed))
            .take(25)
            .map(|(label, id)| choice(&label, id))
            .collect()
    };
    ix.autocomplete(choices).await
}
