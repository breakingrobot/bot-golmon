use super::{
    embed::build_embed,
    items::{self, ItemLine},
    model::{NOTE_LIMIT, Status, TYPE_CHOICES, Transaction, type_label},
    service::{self, NewTransaction},
    templates::{self, NewTemplate, Refused},
};
use crate::modules::{
    audit::{self, Action, Entry},
    contacts,
    permissions::{self, Perm},
};
use crate::{
    discord::{
        commands::{modal_select, modal_text},
        components::{label, select_option, string_select, text_input},
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        messages::{self, parse_id},
        options::Options,
        site::{self, Page},
        suggest,
    },
    util::{
        format::{DESCRIPTION_LIMIT, field, format_price, truncate},
        ids::encode_custom_id,
    },
};
use anyhow::Result;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::modal::ModalInteractionData,
    },
    channel::{
        ChannelType,
        message::{Component, component::TextInputStyle},
    },
    guild::Permissions,
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, IntegerBuilder, NumberBuilder, RoleBuilder, StringBuilder,
    SubCommandBuilder, SubCommandGroupBuilder,
};

pub const CLOSED: &str = "Cette transaction est clôturée et ne peut plus être modifiée.";

fn transaction_option() -> StringBuilder {
    StringBuilder::new("transaction", "Transaction concernée").required(true).autocomplete(true)
}

fn note_option() -> StringBuilder {
    StringBuilder::new("note", "Note affichée dans la fiche (lieu, contact…)").max_length(NOTE_LIMIT as u16)
}

fn template_option(required: bool) -> StringBuilder {
    StringBuilder::new("modele", "Modèle de transaction").required(required).autocomplete(true)
}

pub fn definition() -> Command {
    CommandBuilder::new("transaction", "Gestion des transactions du groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("create", "Crée une transaction")
                .option(StringBuilder::new("groupe", "Groupe concerné").required(true).autocomplete(true))
                .option(StringBuilder::new("type", "Achat ou vente").required(true).choices(TYPE_CHOICES))
                .option(NumberBuilder::new("prix", "Prix total").required(true))
                .option(note_option()),
        )
        .option(
            SubCommandBuilder::new(
                "nouvelle",
                "Saisie guidée d'une transaction (formulaire), depuis un modèle si besoin",
            )
            .option(template_option(false)),
        )
        .option(
            SubCommandBuilder::new("add-item", "Ajoute un objet à une transaction")
                .option(transaction_option())
                .option(StringBuilder::new("nom", "Nom de l'objet").required(true).autocomplete(true))
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
        .option(
            SubCommandGroupBuilder::new("modele", "Modèles de transaction (gradés par défaut)").subcommands([
                SubCommandBuilder::new("creer", "Crée un modèle : type, groupe, prix et objets habituels")
                    .option(
                        StringBuilder::new("nom", "Nom du modèle")
                            .required(true)
                            .max_length(templates::NAME_LIMIT as u16),
                    )
                    .option(StringBuilder::new("type", "Achat ou vente").required(true).choices(TYPE_CHOICES))
                    .option(StringBuilder::new("groupe", "Groupe habituel").autocomplete(true).max_length(100))
                    .option(NumberBuilder::new("prix", "Prix total habituel").min_value(0.0))
                    .option(StringBuilder::new("objets", "Objets : 2x Pistolet @ 15000, Kevlar x5").max_length(1000)),
                SubCommandBuilder::new("liste", "Modèles du serveur"),
                SubCommandBuilder::new("supprimer", "Supprime un modèle").option(template_option(true)),
            ]),
        )
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
    let guild = guild_id.to_string();
    let (name, query) = options.focused.clone().unwrap_or_default();
    match name.as_str() {
        "groupe" => return group_autocomplete(ix, &guild, &query).await,
        "modele" => {
            if !permissions::can(ix, Perm::TransactionUtiliser).await? {
                return ix.autocomplete(Vec::new()).await;
            }
            let candidates = templates::list(&ix.ctx.db, &guild).await?.into_iter().map(|t| {
                let groupe = t.groupe.as_deref().map(|g| format!(" · {g}")).unwrap_or_default();
                (format!("{} · {}{groupe}", t.nom, type_label(&t.kind)), t.id)
            });
            let choices = suggest::filter(candidates, &query);
            return ix.autocomplete(choices.into_iter().map(|(l, v)| choice(&l, v)).collect()).await;
        }
        "nom" => {
            let items = service::known_items(&ix.ctx.db, &guild).await?;
            let choices = suggest::filter(items.into_iter().map(|i| (i.clone(), i)), &query);
            return ix.autocomplete(choices.into_iter().map(|(l, v)| choice(&l, v)).collect()).await;
        }
        _ => {}
    }
    // `view` doit pouvoir retrouver aussi les transactions terminées.
    let open_only = options.sub() != "view";
    let transactions = service::search(&ix.ctx.db, &guild, &query, open_only).await?;
    let choices = transactions
        .iter()
        .map(|t| choice(&format!("{} · {} · {}", t.groupe, format_price(t.prix), t.status().label()), t.id.clone()))
        .collect();
    ix.autocomplete(choices).await
}

/// Groupe : contacts du répertoire (si la personne peut le consulter) et groupes déjà utilisés.
async fn group_autocomplete(ix: &Ix, guild: &str, typed: &str) -> Result<()> {
    if !permissions::can(ix, Perm::TransactionUtiliser).await? {
        return ix.autocomplete(Vec::new()).await;
    }
    let contacts = if permissions::can(ix, Perm::ContactVoir).await? {
        contacts::service::list(&ix.ctx.db, guild, &contacts::service::Filter::default())
            .await?
            .into_iter()
            .map(|c| {
                let emoji = c.relation().emoji();
                (c.name, emoji)
            })
            .collect()
    } else {
        Vec::new()
    };
    let used = service::known_groups(&ix.ctx.db, guild).await?;
    let choices = service::group_choices(contacts, used, typed);
    ix.autocomplete(choices.into_iter().map(|(l, v)| choice(&l, v)).collect()).await
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();
    if options.group() == Some("modele") {
        if !permissions::require(ix, Perm::TransactionModeles).await? {
            return Ok(());
        }
        return match options.sub() {
            "creer" => create_template(ix, options, &guild).await,
            "liste" => list_templates(ix, &guild).await,
            "supprimer" => delete_template(ix, options, &guild).await,
            _ => Ok(()),
        };
    }
    if !permissions::require(ix, Perm::TransactionUtiliser).await? {
        return Ok(());
    }
    match options.sub() {
        "create" => create(ix, options, &guild).await,
        "nouvelle" => new_form(ix, options, &guild).await,
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
            note: clean_note(options.string("note")),
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
    // Seule action irréversible du module : réservée aux gradés par défaut.
    if !permissions::require(ix, Perm::TransactionSupprimer).await? {
        return Ok(());
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
    audit::record(ix, Entry::new(Action::TransactionDelete, None, deleted_details(&transaction))).await;
    ix.edit_text("Transaction supprimée définitivement de la base de données.").await
}

/// Détails du journal pour une transaction supprimée (commande ou bouton).
pub fn deleted_details(transaction: &Transaction) -> String {
    format!("{} — {} — {} $ ({})", transaction.groupe, transaction.kind, transaction.prix, transaction.statut)
}

async fn view(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let id = options.string("transaction").unwrap_or_default();
    let Some(transaction) = service::get(&ix.ctx.db, guild, &id).await? else {
        return ix.reply(Reply::ephemeral("Transaction introuvable.")).await;
    };
    let relation = service::group_relation(&ix.ctx.db, guild, &transaction.groupe).await?;
    let link = match parse_id(guild) {
        Some(guild_id) => site::row_for(&ix.ctx, guild_id, Page::Transactions { groupe: Some(&transaction.groupe) }),
        None => Vec::new(),
    };
    ix.reply(Reply::embed(build_embed(&transaction, relation)).hidden().with_components(link)).await
}

/// Note saisie : vide → aucune, bornée à 500 caractères.
pub fn clean_note(raw: Option<String>) -> Option<String> {
    raw.map(|n| truncate(n.trim(), NOTE_LIMIT)).filter(|n| !n.is_empty())
}

/// Valeurs de départ du formulaire de saisie guidée.
#[derive(Debug, Default, PartialEq)]
pub struct Draft {
    pub groupe: String,
    pub kind: String,
    pub prix: String,
    pub objets: String,
}

impl Draft {
    pub fn from_template(template: &templates::Template) -> Self {
        Self {
            groupe: template.groupe.clone().unwrap_or_default(),
            kind: template.kind.clone(),
            prix: template.prix.map(|p| format_price(p).replace('\u{202f}', " ")).unwrap_or_default(),
            objets: template.objets.replace(", ", "\n"),
        }
    }
}

fn short_input(custom_id: &str, value: &str, placeholder: &str, required: bool, max: u16) -> Component {
    let mut input = text_input(custom_id, TextInputStyle::Short);
    input.required = Some(required);
    input.max_length = Some(max);
    input.placeholder = Some(placeholder.to_owned());
    input.value = (!value.is_empty()).then(|| value.to_owned());
    Component::TextInput(input)
}

fn long_input(custom_id: &str, value: &str, placeholder: &str, max: u16) -> Component {
    let mut input = text_input(custom_id, TextInputStyle::Paragraph);
    input.required = Some(false);
    input.max_length = Some(max);
    input.placeholder = Some(placeholder.to_owned());
    input.value = (!value.is_empty()).then(|| value.to_owned());
    Component::TextInput(input)
}

/// Formulaire de saisie guidée : groupe, type (menu), prix, objets, note.
pub fn new_form_components(draft: &Draft) -> Vec<Component> {
    let types = TYPE_CHOICES
        .iter()
        .map(|(name, value)| {
            let mut option = select_option(*name, *value);
            option.default = draft.kind == *value;
            option
        })
        .collect();
    let mut select = string_select("type".to_owned(), "Achat ou vente", types);
    select.required = Some(true);
    vec![
        label(
            "Groupe",
            Some("Contact du répertoire ou nom libre."),
            short_input("groupe", &draft.groupe, "Ballas", true, 100),
        ),
        label("Type", None, Component::SelectMenu(select)),
        label(
            "Prix total",
            Some("Vide : calculé depuis les objets s'ils ont tous un prix unitaire."),
            short_input("prix", &draft.prix, "150000", false, 20),
        ),
        label(
            "Objets (un par ligne)",
            Some("2x Pistolet @ 15000 · Kevlar x5 · Munitions"),
            long_input("objets", &draft.objets, "2x Pistolet @ 15000\nKevlar x5", 1500),
        ),
        label("Note", None, long_input("note", "", "Lieu, contact, délai…", NOTE_LIMIT as u16)),
    ]
}

async fn new_form(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let draft = match options.string("modele") {
        Some(value) => match templates::resolve(&ix.ctx.db, guild, &value).await? {
            Some(template) => Draft::from_template(&template),
            None => return ix.reply(Reply::ephemeral("Modèle introuvable : choisis-le dans la liste proposée.")).await,
        },
        None => Draft { kind: "VENTE".to_owned(), ..Default::default() },
    };
    ix.modal(
        encode_custom_id("transaction", "nouvelle-modal", "new", None),
        "Nouvelle transaction".to_owned(),
        new_form_components(&draft),
    )
    .await
}

/// Saisie validée : prix total donné, sinon calculé depuis les objets.
pub fn resolve_price(raw: &str, items: &[ItemLine]) -> Result<f64, String> {
    if raw.trim().is_empty() {
        return items::total(items).ok_or_else(|| {
            "Indique le prix total, ou un prix unitaire pour chaque objet (`2x Pistolet @ 15000`).".to_owned()
        });
    }
    items::parse_price(raw).ok_or_else(|| "Prix total illisible, ex : `150000`.".to_owned())
}

/// Formulaire de `/transaction nouvelle` validé.
pub async fn new_modal(ix: &Ix, modal: &ModalInteractionData) -> Result<()> {
    let (Some(guild_id), Some(channel_id)) = (ix.guild_id(), ix.channel_id()) else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !permissions::require(ix, Perm::TransactionUtiliser).await? {
        return Ok(());
    }
    let groupe = modal_text(modal, "groupe").trim().to_owned();
    if groupe.is_empty() || groupe.chars().count() > 100 {
        return ix.reply(Reply::ephemeral("Indique le groupe (100 caractères au plus).")).await;
    }
    let kind = if modal_select(modal, "type").first().map(String::as_str) == Some("ACHAT") { "ACHAT" } else { "VENTE" };
    let lines = match items::parse_items(&modal_text(modal, "objets")) {
        Ok(lines) => lines,
        Err(message) => return ix.reply(Reply::ephemeral(message)).await,
    };
    let prix = match resolve_price(&modal_text(modal, "prix"), &lines) {
        Ok(prix) => prix,
        Err(message) => return ix.reply(Reply::ephemeral(message)).await,
    };
    ix.defer(true).await?;
    let guild = guild_id.to_string();
    let id = service::create(
        &ix.ctx.db,
        NewTransaction {
            guild_id: guild.clone(),
            channel_id: channel_id.to_string(),
            groupe: groupe.clone(),
            kind: kind.to_owned(),
            prix,
            created_by_id: ix.user_id().to_string(),
            note: clean_note(Some(modal_text(modal, "note"))),
        },
    )
    .await?;
    for line in &lines {
        service::add_item(&ix.ctx.db, &id, &line.nom, line.quantite, line.prix_unitaire).await?;
    }
    service::refresh(&ix.ctx, &guild, &id).await?;
    ix.edit_text(format!("Transaction pour **{groupe}** créée : {} · {} objet(s).", format_price(prix), lines.len()))
        .await
}

async fn create_template(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let nom = options.string("nom").unwrap_or_default().trim().to_owned();
    if nom.is_empty() || nom.chars().count() > templates::NAME_LIMIT {
        return ix.reply(Reply::ephemeral("Nom du modèle invalide (60 caractères au plus).")).await;
    }
    let objets = match items::parse_items(&options.string("objets").unwrap_or_default()) {
        Ok(lines) => items::format_items(&lines),
        Err(message) => return ix.reply(Reply::ephemeral(message)).await,
    };
    let kind = options.string("type").filter(|t| t == "ACHAT").unwrap_or_else(|| "VENTE".to_owned());
    let new = NewTemplate {
        nom: nom.clone(),
        kind: kind.clone(),
        groupe: options.string("groupe").map(|g| g.trim().to_owned()).filter(|g| !g.is_empty()),
        prix: options.number("prix").filter(|p| p.is_finite() && *p >= 0.0),
        objets: objets.clone(),
        created_by_id: ix.user_id().to_string(),
    };
    match templates::create(&ix.ctx.db, guild, new).await? {
        Ok(template) => {
            let details = format!(
                "Créé : **{}** — {}{}",
                template.nom,
                type_label(&kind),
                if objets.is_empty() { String::new() } else { format!(" — {objets}") }
            );
            audit::record(ix, Entry::new(Action::TransactionTemplate, None, details)).await;
            ix.reply(Reply::ephemeral(format!(
                "Modèle **{nom}** créé. Utilise-le avec `/transaction nouvelle modele:{nom}`."
            )))
            .await
        }
        Err(Refused::NameTaken) => ix.reply(Reply::ephemeral("Un modèle porte déjà ce nom.")).await,
        Err(Refused::TooMany) => {
            let message = format!(
                "{} modèles au plus : supprimes-en un (`/transaction modele supprimer`).",
                templates::MAX_TEMPLATES
            );
            ix.reply(Reply::ephemeral(message)).await
        }
    }
}

/// Ligne d'un modèle dans `/transaction modele liste`.
pub fn template_line(template: &templates::Template) -> String {
    let mut parts = vec![format!("**{}**", template.nom), type_label(&template.kind).to_owned()];
    if let Some(groupe) = &template.groupe {
        parts.push(groupe.clone());
    }
    if let Some(prix) = template.prix {
        parts.push(format_price(prix));
    }
    let mut line = format!("• {}", parts.join(" · "));
    if !template.objets.is_empty() {
        line.push_str(&format!("\n  {}", truncate(&template.objets, 300)));
    }
    line
}

async fn list_templates(ix: &Ix, guild: &str) -> Result<()> {
    let list = templates::list(&ix.ctx.db, guild).await?;
    let description = if list.is_empty() {
        "Aucun modèle. Crée-en un avec `/transaction modele creer`.".to_owned()
    } else {
        truncate(&list.iter().map(template_line).collect::<Vec<_>>().join("\n"), DESCRIPTION_LIMIT)
    };
    let embed = EmbedBuilder::new()
        .title("🧩 Modèles de transaction")
        .color(0x5865f2)
        .description(description)
        .footer(footer("Utilisation : /transaction nouvelle modele:…"))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn delete_template(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    let Some(template) = templates::resolve(&ix.ctx.db, guild, &options.string("modele").unwrap_or_default()).await?
    else {
        return ix.reply(Reply::ephemeral("Modèle introuvable : choisis-le dans la liste proposée.")).await;
    };
    templates::delete(&ix.ctx.db, guild, &template.id).await?;
    audit::record(ix, Entry::new(Action::TransactionTemplate, None, format!("Supprimé : **{}**", template.nom))).await;
    ix.reply(Reply::ephemeral(format!("Modèle **{}** supprimé.", template.nom))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> templates::Template {
        templates::Template {
            id: "t1".into(),
            nom: "Armes Ballas".into(),
            kind: "ACHAT".into(),
            groupe: Some("Ballas".into()),
            prix: Some(150_000.0),
            objets: "2x Pistolet @ 15 000, 5x Kevlar".into(),
        }
    }

    #[test]
    fn prefills_the_form_from_a_template() {
        let draft = Draft::from_template(&template());
        assert_eq!(
            draft,
            Draft {
                groupe: "Ballas".into(),
                kind: "ACHAT".into(),
                prix: "150 000".into(),
                objets: "2x Pistolet @ 15 000\n5x Kevlar".into(),
            }
        );
        let components = new_form_components(&draft);
        assert_eq!(components.len(), 5, "un modal a 5 champs au plus");
        let Component::Label(label) = &components[1] else { panic!("libellé attendu") };
        let Component::SelectMenu(select) = label.component.as_ref() else { panic!("menu attendu") };
        let chosen: Vec<&str> =
            select.options.iter().flatten().filter(|o| o.default).map(|o| o.value.as_str()).collect();
        assert_eq!(chosen, ["ACHAT"]);
        // Les objets du modèle se relisent tels quels.
        assert_eq!(items::parse_items(&draft.objets).unwrap().len(), 2);
    }

    #[test]
    fn price_from_items_when_left_empty() {
        let lines = items::parse_items("2x Pistolet @ 15000\nKevlar x5 @ 1000").unwrap();
        assert_eq!(resolve_price("", &lines), Ok(35_000.0));
        assert_eq!(resolve_price("40 000", &lines), Ok(40_000.0));
        assert!(resolve_price("", &items::parse_items("Kevlar").unwrap()).unwrap_err().contains("prix unitaire"));
        assert!(resolve_price("beaucoup", &lines).is_err());
        assert_eq!(clean_note(Some("  ".into())), None);
    }

    #[test]
    fn lists_templates() {
        assert_eq!(
            template_line(&template()),
            "• **Armes Ballas** · Achat · Ballas · 150\u{202f}000\n  2x Pistolet @ 15 000, 5x Kevlar"
        );
    }
}
