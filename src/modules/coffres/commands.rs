use super::{
    alerts::{self, MAX_QUANTITY},
    dashboard, ingest,
    parser::{Kind, item_label},
    service::{self, Filter, StockLine},
};
use crate::{
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        messages::{self, parse_id},
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
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker},
    },
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, IntegerBuilder, RoleBuilder, StringBuilder, SubCommandBuilder,
    UserBuilder,
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
        .option(
            SubCommandBuilder::new("seuil", "Seuil de stock bas d'un objet (0 : retirer le seuil)")
                .option(StringBuilder::new("objet", "Objet surveillé").required(true).autocomplete(true))
                .option(
                    IntegerBuilder::new("min", "Alerte quand le stock passe sous cette quantité (0 : retirer)")
                        .required(true)
                        .min_value(0)
                        .max_value(MAX_QUANTITY),
                )
                .option(StringBuilder::new("propriete", "Une seule propriété (défaut : toutes)").autocomplete(true)),
        )
        .option(SubCommandBuilder::new("seuils", "Seuils de stock bas et état actuel du stock"))
        .build()
}

/// `/config coffre`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("coffre", "Coffres : webhook du jeu, alertes, tableau du stock (sans option : affiche)")
        .option(
            ChannelBuilder::new("salon", "Salon où le jeu publie les mouvements des coffres")
                .channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("retirer", "Ne plus suivre ce salon"))
        .option(
            ChannelBuilder::new("alertes_salon", "Salon des alertes (stock bas, gros retraits…)")
                .channel_types([ChannelType::GuildText]),
        )
        .option(RoleBuilder::new("alertes_role", "Rôle mentionné dans les alertes"))
        .option(
            IntegerBuilder::new("gros_retrait", "Alerte pour un retrait d'au moins cette quantité (0 : désactiver)")
                .min_value(0)
                .max_value(MAX_QUANTITY),
        )
        .option(BooleanBuilder::new("joueur_inconnu", "Alerte quand un personnage relié à aucun membre retire"))
        .option(BooleanBuilder::new("alertes_desactiver", "Ne plus publier d'alertes (réglages conservés)"))
        .option(
            ChannelBuilder::new("tableau", "Salon du tableau permanent du stock")
                .channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("sans_tableau", "Retirer le tableau du stock"))
}

/// Réglages actuels, pour `/config coffre` sans option.
async fn config_summary(ix: &Ix, guild: &str) -> Result<String> {
    let channels = service::channels(&ix.ctx.db, guild).await?;
    let mut text = if channels.is_empty() {
        "Aucun salon suivi. Ajoute celui du webhook avec `/config coffre salon:#…`.".to_owned()
    } else {
        format!(
            "Salons suivis : {}.",
            channels
                .iter()
                .map(|c| format!(
                    "<#{}>{}",
                    c.channel_id,
                    if c.webhook_id.is_some() { "" } else { " (webhook du jeu pas encore vu)" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let alerts = alerts::get_alerts(&ix.ctx.db, guild).await?;
    text.push_str(&match &alerts.channel_id {
        Some(channel) => format!(
            "\nAlertes dans <#{channel}>{} : {} seuil(s) de stock bas, gros retrait {}, personnage inconnu {}.",
            alerts.role_id.as_ref().map(|r| format!(" (mention <@&{r}>)")).unwrap_or_default(),
            alerts.thresholds.len(),
            alerts.big_withdrawal.map_or_else(|| "désactivé".to_owned(), |q| format!("dès {q}")),
            if alerts.unknown_player { "activé" } else { "désactivé" }
        ),
        None => "\nAlertes désactivées (`/config coffre alertes_salon:#…`).".to_owned(),
    });
    let (board, _) = dashboard::get_dashboard(&ix.ctx.db, guild).await?;
    text.push_str(&match board {
        Some(channel) => format!("\nTableau du stock publié dans <#{channel}>."),
        None => "\nPas de tableau du stock (`/config coffre tableau:#…`).".to_owned(),
    });
    Ok(text)
}

/// Options d'alerte : renvoie le compte rendu, ou `None` si aucune n'est saisie.
async fn config_alerts(ix: &Ix, options: &Options, guild: &str) -> Result<Option<String>> {
    let channel = options.channel("alertes_salon");
    let role = options.role("alertes_role");
    let big = options.integer("gros_retrait");
    let unknown = options.boolean("joueur_inconnu");
    let disable = options.boolean("alertes_desactiver") == Some(true);
    if channel.is_none() && role.is_none() && big.is_none() && unknown.is_none() && !disable {
        return Ok(None);
    }
    let mut alerts = alerts::get_alerts(&ix.ctx.db, guild).await?;
    if let Some(channel) = channel {
        alerts.channel_id = Some(channel.to_string());
    }
    if disable {
        alerts.channel_id = None;
    }
    if let Some(role) = role {
        alerts.role_id = Some(role.to_string());
    }
    if let Some(big) = big {
        alerts.big_withdrawal = (big > 0).then_some(big);
    }
    if let Some(unknown) = unknown {
        alerts.unknown_player = unknown;
    }
    alerts::set_alerts(&ix.ctx.db, guild, &alerts).await?;
    Ok(Some(match &alerts.channel_id {
        Some(channel) => format!(
            "Alertes publiées dans <#{channel}>{} · gros retrait : {} · personnage inconnu : {} · seuils : `/coffre seuil`.",
            alerts.role_id.as_ref().map(|r| format!(", en mentionnant <@&{r}>")).unwrap_or_default(),
            alerts.big_withdrawal.map_or_else(|| "désactivé".to_owned(), |q| format!("dès {q}")),
            if alerts.unknown_player { "activé" } else { "désactivé" }
        ),
        None => "Les alertes des coffres ne sont plus publiées (réglages conservés).".to_owned(),
    }))
}

/// Options du tableau du stock : renvoie le compte rendu, ou `None` si aucune n'est saisie.
async fn config_dashboard(ix: &Ix, options: &Options, guild: &str) -> Result<Option<String>> {
    let channel = options.channel("tableau").map(|id| id.to_string());
    if channel.is_none() && options.boolean("sans_tableau") != Some(true) {
        return Ok(None);
    }
    // L'ancien message disparaît : changement de salon ou retrait du tableau.
    if let (Some(old_channel), Some(old_message)) = dashboard::get_dashboard(&ix.ctx.db, guild).await?
        && let (Some(c), Some(m)) = (parse_id(&old_channel), parse_id(&old_message))
    {
        messages::delete_quietly(&ix.ctx, c, m).await;
    }
    dashboard::set_dashboard_channel(&ix.ctx.db, guild, channel.as_deref()).await?;
    Ok(Some(match channel {
        Some(channel) => {
            dashboard::sync_dashboard(&ix.ctx, guild).await?;
            format!("Tableau du stock publié dans <#{channel}> (mis à jour au plus une fois par minute).")
        }
        None => "Le tableau du stock n'est plus publié.".to_owned(),
    }))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    if options.summary().is_empty() {
        return ix.reply(Reply::ephemeral(config_summary(ix, guild).await?)).await;
    }
    ix.defer(true).await?;
    let mut lines = Vec::new();
    if let Some(channel_id) = options.channel("salon") {
        lines.push(config_channel(ix, options, guild, channel_id).await?);
    }
    lines.extend(config_alerts(ix, options, guild).await?);
    lines.extend(config_dashboard(ix, options, guild).await?);
    if lines.is_empty() {
        lines.push("Indique le salon concerné (`salon:#…`).".to_owned());
    }
    ix.edit_text(lines.join("\n")).await
}

/// Ajout ou retrait d'un salon suivi.
async fn config_channel(ix: &Ix, options: &Options, guild: &str, channel_id: Id<ChannelMarker>) -> Result<String> {
    if options.boolean("retirer") == Some(true) {
        let removed = service::remove_channel(&ix.ctx.db, guild, &channel_id.to_string()).await?;
        return Ok(if removed {
            format!("<#{channel_id}> n'est plus suivi. Les mouvements déjà lus sont conservés.")
        } else {
            format!("<#{channel_id}> n'était pas suivi.")
        });
    }
    if !ix.ctx.message_content {
        return Ok("Le bot ne peut pas lire les messages du webhook : active *Message Content Intent* dans le \
                   portail développeur Discord (onglet Bot), puis redémarre le bot."
            .to_owned());
    }
    service::add_channel(&ix.ctx.db, guild, &channel_id.to_string()).await?;
    // Première lecture : l'historique récent du salon (jusqu'à 5 000 messages) ;
    // salon déjà suivi : les messages publiés depuis la dernière lecture.
    let added = match service::channel(&ix.ctx.db, &channel_id.to_string()).await? {
        Some(mut channel) => ingest::catch_up(&ix.ctx, &mut channel).await?,
        None => 0,
    };
    Ok(format!(
        "<#{channel_id}> est suivi : chaque dépôt et retrait publié par le jeu est enregistré. \
         {added} mouvement(s) importé(s) depuis l'historique du salon."
    ))
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let perm = if matches!(options.sub(), "lier" | "nommer" | "seuil") { Perm::CoffreGerer } else { Perm::CoffreVoir };
    if !permissions::require(ix, perm).await? {
        return Ok(());
    }
    match options.sub() {
        "stock" => stock(ix, options, guild_id).await,
        "historique" => history(ix, options, guild_id).await,
        "lier" => link(ix, options, guild_id).await,
        "nommer" => rename(ix, options, guild_id).await,
        "seuil" => set_threshold(ix, options, guild_id).await,
        "seuils" => list_thresholds(ix, guild_id).await,
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
    let thresholds = alerts::thresholds(&ix.ctx.db, &guild).await?;
    let description = if grouped.is_empty() {
        "Aucun objet en stock d'après les mouvements enregistrés.".to_owned()
    } else {
        truncate(
            &grouped
                .iter()
                .map(|(property, lines)| {
                    let items = lines
                        .iter()
                        .map(|l| match thresholds.min_for(&l.property_id, &l.item) {
                            Some(min) if l.quantity() < min => {
                                format!("• ⚠️ {} × **{}** *(seuil {min})*", item_label(&l.item), l.quantity())
                            }
                            _ => format!("• {} × **{}**", item_label(&l.item), l.quantity()),
                        })
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
    if !permissions::require_member(ix, member, &options.user_name(member)).await? {
        return Ok(());
    }
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

async fn set_threshold(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let item = options.string("objet").unwrap_or_default().trim().to_owned();
    let min = options.integer("min").unwrap_or(0);
    let property = options.string("propriete").map(|p| p.trim().to_owned()).filter(|p| !p.is_empty());
    if item.is_empty() || item.chars().count() > 100 || !(0..=MAX_QUANTITY).contains(&min) {
        return ix.reply(Reply::ephemeral("Objet ou quantité invalide.")).await;
    }
    if min > 0 && alerts::list_thresholds(&ix.ctx.db, &guild).await?.len() >= alerts::MAX_THRESHOLDS {
        return ix.reply(Reply::ephemeral(format!("{} seuils au plus.", alerts::MAX_THRESHOLDS))).await;
    }
    let names = property_names(ix, &guild).await?;
    let place = property.as_deref().map_or_else(|| "toutes les propriétés".to_owned(), |p| property_label(&names, p));
    let changed = alerts::set_threshold(&ix.ctx.db, &guild, property.as_deref(), &item, min).await?;
    let text = match (min, changed) {
        (0, false) => return ix.reply(Reply::ephemeral("Aucun seuil à retirer pour cet objet.")).await,
        (0, true) => format!("Seuil retiré : **{}** ({place}).", item_label(&item)),
        _ => format!("Alerte quand **{}** passe sous **{min}** ({place}).", item_label(&item)),
    };
    audit::record(ix, Entry::new(Action::Config, None, format!("**coffre seuil** — {text}"))).await;
    dashboard::request_refresh(&ix.ctx, &guild);
    let mut reply = text;
    if alerts::get_alerts(&ix.ctx.db, &guild).await?.channel_id.is_none() {
        reply.push_str("\nLes alertes ne sont publiées dans aucun salon : `/config coffre alertes_salon:#…`.");
    }
    ix.reply(Reply::ephemeral(reply)).await
}

async fn list_thresholds(ix: &Ix, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let list = alerts::list_thresholds(&ix.ctx.db, &guild).await?;
    let names = property_names(ix, &guild).await?;
    let stock = service::stock(&ix.ctx.db, &guild, None).await?;
    let thresholds = alerts::Thresholds::new(&list);
    let description = if list.is_empty() {
        "Aucun seuil. Ajoute-en un avec `/coffre seuil objet:… min:…`.".to_owned()
    } else {
        let lines: Vec<String> = list
            .iter()
            .map(|t| {
                let place = t
                    .property_id
                    .as_deref()
                    .map_or_else(|| "toutes les propriétés".to_owned(), |p| property_label(&names, p));
                // Propriétés où l'objet est sous ce seuil (celui qui s'y applique).
                let low: Vec<String> = stock
                    .iter()
                    .filter(|l| l.item == t.item && t.property_id.as_ref().is_none_or(|p| *p == l.property_id))
                    .filter(|l| thresholds.min_for(&l.property_id, &l.item) == Some(t.min))
                    .filter(|l| l.quantity() < t.min)
                    .map(|l| format!("{} ({})", property_label(&names, &l.property_id), l.quantity()))
                    .collect();
                let state = if low.is_empty() {
                    "✅".to_owned()
                } else {
                    format!("⚠️ sous le seuil : {}", low.join(", "))
                };
                format!("• **{}** < {} — {place} · {state}", item_label(&t.item), t.min)
            })
            .collect();
        truncate(&lines.join("\n"), DESCRIPTION_LIMIT)
    };
    let alerts = alerts::get_alerts(&ix.ctx.db, &guild).await?;
    let footer_text = match alerts.channel_id {
        Some(_) => "Alertes actives · réglages : /config coffre".to_owned(),
        None => "Alertes désactivées : /config coffre alertes_salon:#…".to_owned(),
    };
    let embed = EmbedBuilder::new()
        .title("📉 Seuils de stock bas")
        .color(COLOR)
        .description(description)
        .footer(footer(footer_text))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
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
