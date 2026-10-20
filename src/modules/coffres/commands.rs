use super::{
    alerts::{self, MAX_QUANTITY},
    dashboard, ingest,
    parser::{Kind, item_label},
    pending::{self, Pending, Taken},
    service::{self, Correction, Filter, StockLine},
};
use crate::{
    discord::{
        commands::modal_text,
        components::{ButtonExt as _, button, label, row, text_input},
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        messages::{self, parse_id},
        options::Options,
        site::{self, Page},
        suggest,
    },
    modules::{
        audit::{self, Action, Entry},
        permissions::{self, Perm},
    },
    util::{
        format::{DESCRIPTION_LIMIT, truncate},
        ids::{CustomId, encode_custom_id},
        time::{DbTime, format_date_label},
    },
};
use anyhow::Result;
use chrono::Duration;
use std::collections::{BTreeMap, HashMap};
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::modal::ModalInteractionData,
    },
    channel::{
        ChannelType,
        message::{
            Component,
            component::{ButtonStyle, TextInputStyle},
        },
    },
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
                .option(
                    StringBuilder::new("id_jeu", "Identifiant du personnage, ex : 340772")
                        .required(true)
                        .autocomplete(true),
                )
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
        .option(
            SubCommandBuilder::new(
                "inventaire",
                "Quantité réelle comptée en jeu (sans objet ni liste : coller l'inventaire, avec aperçu)",
            )
            .option(property_option(true))
            .option(StringBuilder::new("objet", "Un objet, ex : repair_kit").autocomplete(true))
            .option(
                IntegerBuilder::new("quantite", "Quantité réellement présente dans le coffre")
                    .min_value(0)
                    .max_value(service::MAX_INVENTORY),
            )
            .option(StringBuilder::new("liste", "Plusieurs objets : repair_kit:42, acetone:2790, …").max_length(1800)),
        )
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
        .option(BooleanBuilder::new(
            "relire",
            "Relit l'historique du salon (5 000 messages) : ajoute seulement ce qui manque, ne retire rien",
        ))
        .option(
            StringBuilder::new("oublier_webhook", "Identifiant d'un webhook à ne plus lire (voir `/config coffre`)")
                .autocomplete(true),
        )
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
    let webhooks = service::webhooks(&ix.ctx.db, guild, None).await?;
    let mut text = if channels.is_empty() {
        "Aucun salon suivi. Ajoute celui du webhook avec `/config coffre salon:#…`.".to_owned()
    } else {
        let lines: Vec<String> = channels
            .iter()
            .map(|c| {
                let known: Vec<String> = webhooks
                    .iter()
                    .filter(|w| w.channel_id == c.channel_id)
                    .map(|w| format!("« {} » (`{}`)", w.name.as_deref().unwrap_or("webhook"), w.webhook_id))
                    .collect();
                if known.is_empty() {
                    format!("• <#{}> : aucun webhook du jeu encore vu", c.channel_id)
                } else {
                    format!("• <#{}> : {}", c.channel_id, known.join(", "))
                }
            })
            .collect();
        format!("Salons suivis et webhooks reconnus :\n{}", lines.join("\n"))
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
    if let Some(webhook) = options.string("oublier_webhook").map(|w| w.trim().to_owned()).filter(|w| !w.is_empty()) {
        lines.push(if service::remove_webhook(&ix.ctx.db, guild, &webhook).await? {
            format!(
                "Webhook `{webhook}` oublié : ses prochains messages sont ignorés (les mouvements déjà lus restent). \
                 Il sera reconnu de nouveau s'il publie encore un encadré « Coffre »."
            )
        } else {
            format!("Aucun webhook `{webhook}` reconnu sur ce serveur (voir `/config coffre`).")
        });
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
    // Première lecture (ou `relire`) : l'historique récent du salon, jusqu'à
    // 5 000 messages, en ajout seulement (un message déjà lu n'est jamais
    // recompté, rien n'est retiré) ; sinon les messages publiés depuis la
    // dernière lecture.
    let added = match service::channel(&ix.ctx.db, &channel_id.to_string()).await? {
        Some(mut channel) => {
            if options.boolean("relire") == Some(true) {
                channel.last_message_id = None;
            }
            ingest::catch_up(&ix.ctx, &mut channel).await?
        }
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
    let perm = if matches!(options.sub(), "lier" | "nommer" | "seuil" | "inventaire") {
        Perm::CoffreGerer
    } else {
        Perm::CoffreVoir
    };
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
        "inventaire" => inventory(ix, options, guild_id).await,
        _ => Ok(()),
    }
}

/// Espace de noms des composants des coffres (inventaire collé).
pub const NAMESPACE: &str = "coffre";
const INVALID_PROPERTY: &str = "Propriété invalide : son numéro, ex : `8740`.";

fn valid_property(property: &str) -> bool {
    !property.is_empty() && property.len() <= 30 && property.bytes().all(|b| b.is_ascii_digit())
}

/// Inventaire : quantités réellement comptées en jeu ; les écarts deviennent
/// des corrections signées, journalisées. Sans objet ni liste : formulaire
/// pour coller l'inventaire du jeu, avec aperçu avant validation.
async fn inventory(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let property = options.string("propriete").unwrap_or_default().trim().to_owned();
    if !valid_property(&property) {
        return ix.reply(Reply::ephemeral(INVALID_PROPERTY)).await;
    }
    let counted = match (options.string("objet"), options.integer("quantite"), options.string("liste")) {
        (None, None, None) => return paste_modal(ix, &guild, &property).await,
        (Some(item), Some(quantity), None) => match service::normalize_item(&item) {
            Some(item) => vec![(item, quantity)],
            None => return ix.reply(Reply::ephemeral("Nom d'objet invalide, ex : `repair_kit`.")).await,
        },
        (None, None, Some(list)) => match service::parse_inventory(&list) {
            Ok(counted) => counted,
            Err(message) => return ix.reply(Reply::ephemeral(message)).await,
        },
        _ => {
            return ix
                .reply(Reply::ephemeral(
                    "Indique soit `objet` et `quantite`, soit `liste` (ex : `repair_kit:42, acetone:2790`).",
                ))
                .await;
        }
    };
    ix.defer(true).await?;
    let text = apply_inventory(ix, guild_id, &property, &counted).await?;
    ix.edit_text(text).await
}

/// Enregistre un inventaire (écarts recalculés sur le stock du moment),
/// journalise, rafraîchit le tableau ; renvoie le compte rendu.
async fn apply_inventory(
    ix: &Ix,
    guild_id: Id<GuildMarker>,
    property: &str,
    counted: &[(String, i64)],
) -> Result<String> {
    let guild = guild_id.to_string();
    let author = crate::discord::members::display_name(&ix.ctx, guild_id, ix.user_id())
        .unwrap_or_else(|| ix.user_id().to_string());
    let corrections =
        service::set_inventory(&ix.ctx.db, &guild, property, counted, &ix.user_id().to_string(), &author).await?;
    let names = property_names(ix, &guild).await?;
    let label = property_label(&names, property);
    if corrections.is_empty() {
        return Ok(format!("{label} : le stock connu correspondait déjà, rien à corriger."));
    }
    let lines: Vec<String> = corrections.iter().map(correction_line).collect();
    audit::record(ix, Entry::new(Action::CoffreInventory, None, format!("**{label}** — {}", lines.join(", ")))).await;
    super::dashboard::request_refresh(&ix.ctx, &guild);
    Ok(truncate(
        &format!("🧮 Inventaire de {label} : {} correction(s).\n{}", corrections.len(), lines.join("\n")),
        2000,
    ))
}

fn correction_line(c: &Correction) -> String {
    format!("**{}** : {} → {} ({:+})", item_label(&c.item), c.before, c.after, c.delta())
}

/// Écarts entre le stock connu (par objet) et les quantités comptées :
/// corrections à enregistrer, et nombre d'objets déjà justes.
pub fn inventory_diff(known: &HashMap<String, i64>, counted: &[(String, i64)]) -> (Vec<Correction>, usize) {
    let mut unchanged = 0;
    let mut corrections = Vec::new();
    for (item, after) in counted {
        let before = known.get(item).copied().unwrap_or(0);
        if before == *after {
            unchanged += 1;
        } else {
            corrections.push(Correction { item: item.clone(), before, after: *after });
        }
    }
    (corrections, unchanged)
}

/// Aperçu d'un inventaire collé, avant « Valider ».
pub fn preview_text(label: &str, corrections: &[Correction], unchanged: usize) -> String {
    let mut text = format!("🧮 **Inventaire de {label}** — aperçu\n");
    if corrections.is_empty() {
        text.push_str(&format!("Le stock connu correspond déjà ({unchanged} objet(s)) : rien à corriger."));
        return text;
    }
    let lines: Vec<String> = corrections.iter().map(correction_line).collect();
    text.push_str(&crate::util::format::bounded_lines(&lines, 1500, |hidden| {
        format!("… et {hidden} autre(s) écart(s)")
    }));
    text.push_str(&format!("\n{} correction(s)", corrections.len()));
    if unchanged > 0 {
        text.push_str(&format!(" · {unchanged} objet(s) inchangé(s)"));
    }
    text.push_str(".\n-# Rien n'est enregistré avant « Valider ». Le stock sera recompté au moment de valider.");
    text
}

/// Formulaire pour coller l'inventaire du jeu.
async fn paste_modal(ix: &Ix, guild: &str, property: &str) -> Result<()> {
    let names = property_names(ix, guild).await?;
    let mut input = text_input("liste", TextInputStyle::Paragraph);
    input.max_length = Some(4000);
    input.placeholder = Some("repair_kit x42\nacetone: 2790\n650x red_powder".to_owned());
    ix.modal(
        encode_custom_id(NAMESPACE, "inventaire-modal", property, None),
        format!("Inventaire — {}", property_label(&names, property)),
        vec![label(
            "Colle l'inventaire du jeu (une ligne par objet)",
            Some("objet:42, objet x42, 42x objet… Les objets absents de la liste ne changent pas."),
            Component::TextInput(input),
        )],
    )
    .await
}

/// Inventaire collé : aperçu des écarts et boutons « Valider » / « Annuler ».
pub async fn inventory_modal(ix: &Ix, property: &str, modal: &ModalInteractionData) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !permissions::require(ix, Perm::CoffreGerer).await? {
        return Ok(());
    }
    if !valid_property(property) {
        return ix.reply(Reply::ephemeral(INVALID_PROPERTY)).await;
    }
    let counted = match service::parse_inventory(&modal_text(modal, "liste")) {
        Ok(counted) if counted.len() > service::MAX_INVENTORY_ITEMS => {
            let message = format!("{} objets au plus par inventaire.", service::MAX_INVENTORY_ITEMS);
            return ix.reply(Reply::ephemeral(message)).await;
        }
        Ok(counted) => counted,
        Err(message) => return ix.reply(Reply::ephemeral(message)).await,
    };
    let guild = guild_id.to_string();
    let known: HashMap<String, i64> = service::stock(&ix.ctx.db, &guild, Some(property))
        .await?
        .into_iter()
        .map(|l| (l.item.clone(), l.quantity()))
        .collect();
    let (corrections, unchanged) = inventory_diff(&known, &counted);
    let names = property_names(ix, &guild).await?;
    let text = preview_text(&property_label(&names, property), &corrections, unchanged);
    if corrections.is_empty() {
        return ix.reply(Reply::ephemeral(text)).await;
    }
    let key = pending::put(Pending {
        guild_id: guild,
        user_id: ix.user_id().to_string(),
        property_id: property.to_owned(),
        counted,
    });
    let buttons = row(vec![
        Component::Button(
            button(encode_custom_id(NAMESPACE, "inv-ok", &key, None), "Valider", ButtonStyle::Success).with_emoji("✅"),
        ),
        Component::Button(button(encode_custom_id(NAMESPACE, "inv-no", &key, None), "Annuler", ButtonStyle::Secondary)),
    ]);
    ix.reply(Reply::ephemeral(text).with_components(vec![buttons])).await
}

/// Boutons de l'aperçu : « Valider » enregistre, « Annuler » oublie la saisie.
pub async fn button_pressed(ix: &Ix, custom_id: &CustomId<'_>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !matches!(custom_id.action, "inv-ok" | "inv-no") {
        return Ok(());
    }
    let pending = match pending::take(custom_id.id, &ix.user_id().to_string()) {
        Taken::Found(pending) if pending.guild_id == guild_id.to_string() => pending,
        Taken::NotYours => return ix.reply(Reply::ephemeral("Cette saisie appartient à quelqu'un d'autre.")).await,
        _ => {
            return ix
                .update(Reply::text(
                    "Cette saisie a expiré (15 minutes) ou a déjà été traitée : relance `/coffre inventaire propriete:…`.",
                ))
                .await;
        }
    };
    if custom_id.action == "inv-no" {
        return ix.update(Reply::text("Inventaire annulé : rien n'a été enregistré.")).await;
    }
    // Le niveau a pu changer depuis l'aperçu.
    if !permissions::require(ix, Perm::CoffreGerer).await? {
        return Ok(());
    }
    ix.defer_update().await?;
    let text = apply_inventory(ix, guild_id, &pending.property_id, &pending.counted).await?;
    ix.edit(Reply::text(text)).await
}

/// `/config coffre oublier_webhook` : webhooks reconnus (« Gérer le serveur »).
pub async fn config_autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.autocomplete(Vec::new()).await;
    }
    let typed = options.focused.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
    let webhooks = service::webhooks(&ix.ctx.db, &guild_id.to_string(), None).await?;
    let candidates = webhooks.into_iter().map(|w| {
        let channel = parse_id::<ChannelMarker>(&w.channel_id)
            .and_then(|id| ix.ctx.cache.channel(id))
            .and_then(|c| c.name.clone())
            .map(|name| format!(" · #{name}"))
            .unwrap_or_default();
        (format!("« {} » ({}){channel}", w.name.as_deref().unwrap_or("webhook"), w.webhook_id), w.webhook_id)
    });
    let choices = suggest::filter(candidates, &typed).into_iter().map(|(l, v)| choice(&l, v)).collect();
    ix.autocomplete(choices).await
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
    let link = site::row_for(&ix.ctx, guild_id, Page::Coffres { propriete: property.as_deref(), objet: None });
    ix.reply(Reply::embed(embed).hidden().with_components(link)).await
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
                    let (sign, amount) = match r.kind() {
                        Kind::Deposit => ("📥", r.amount.to_string()),
                        Kind::Withdraw => ("📤", r.amount.to_string()),
                        Kind::Adjust => ("🧮", format!("{:+} (correction)", r.amount)),
                    };
                    format!(
                        "{sign} `{}` **{}** × {amount} — {} · {}",
                        format_date_label(r.at),
                        item_label(&r.item),
                        r.player_name,
                        property_label(&names, &r.property_id)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let embed = EmbedBuilder::new()
        .title("📦 Mouvements des coffres")
        .color(COLOR)
        .description(description)
        .footer(footer(format!("{total} mouvement(s) au total")))
        .build();
    let page = Page::Coffres { propriete: filter.property_id.as_deref(), objet: filter.item.as_deref() };
    ix.reply(Reply::embed(embed).hidden().with_components(site::row_for(&ix.ctx, guild_id, page))).await
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
    if name == "id_jeu" {
        // Relier un personnage demande `coffre.gerer`.
        if !permissions::can(ix, Perm::CoffreGerer).await? {
            return ix.autocomplete(Vec::new()).await;
        }
        let players = service::players_seen(&ix.ctx.db, &guild).await?;
        let choices = service::player_choices(players, &typed).into_iter().map(|(l, v)| choice(&l, v)).collect();
        return ix.autocomplete(choices).await;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_only_the_differences() {
        let known: HashMap<String, i64> = [("repair_kit".to_owned(), 8), ("lockpick".to_owned(), 3)].into();
        let counted = service::parse_inventory("repair_kit x42\nlockpick: 3\n2790x acetone").unwrap();
        let (corrections, unchanged) = inventory_diff(&known, &counted);
        assert_eq!(unchanged, 1);
        assert_eq!(
            corrections,
            [
                Correction { item: "repair_kit".into(), before: 8, after: 42 },
                Correction { item: "acetone".into(), before: 0, after: 2790 },
            ]
        );
        let text = preview_text("Planque du port", &corrections, unchanged);
        assert!(text.starts_with("🧮 **Inventaire de Planque du port** — aperçu\n**Repair kit** : 8 → 42 (+34)"));
        assert!(text.contains("**Acetone** : 0 → 2790 (+2790)\n2 correction(s) · 1 objet(s) inchangé(s)."));
        assert!(text.contains("Rien n'est enregistré avant « Valider »"));
        assert!(preview_text("P", &[], 4).contains("correspond déjà (4 objet(s))"));
    }

    #[test]
    fn long_previews_stay_in_a_message() {
        let corrections: Vec<Correction> =
            (0..300).map(|i| Correction { item: format!("objet_{i}"), before: 0, after: i }).collect();
        let text = preview_text("Propriété 8740", &corrections, 0);
        assert!(text.chars().count() <= 2000, "{} caractères", text.chars().count());
        assert!(text.contains("autre(s) écart(s)"));
    }

    #[test]
    fn validates_property_numbers() {
        assert!(valid_property("8740"));
        assert!(!valid_property("87a0") && !valid_property("") && !valid_property(&"1".repeat(31)));
    }
}
