use super::{
    DESCRIPTION_LIMIT, Kind, NAME_LIMIT, NAME_TAKEN, NOTE_LIMIT, PHONE_LIMIT, Relation, optional_text,
    service::{self, Contact, Filter, NewContact},
    validate, validate_note,
};
use crate::{
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        messages::{self, parse_id},
        options::Options,
    },
    modules::audit::{self, Action, Entry},
    modules::permissions::{self, Perm},
    util::{
        format::{self as fmt, ListBlock, field, pack_blocks, truncate},
        time::format_day_label,
    },
};
use anyhow::Result;
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
    id::{Id, marker::GuildMarker},
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, StringBuilder, SubCommandBuilder,
};

const NOT_FOUND: &str = "Contact introuvable : choisis-le dans la liste proposée.";
/// Notes affichées par `/contact voir`.
const NOTES_SHOWN: i64 = 10;
const COLOR: u32 = 0x5865f2;

fn contact_option(description: &str) -> StringBuilder {
    StringBuilder::new("contact", description).required(true).autocomplete(true)
}

fn kind_option(description: &str) -> StringBuilder {
    StringBuilder::new("type", description).choices(Kind::ALL.map(|k| (k.label(), k.as_str())))
}

fn relation_option(description: &str) -> StringBuilder {
    StringBuilder::new("relation", description).choices(Relation::ALL.map(|r| (r.label(), r.as_str())))
}

fn phone_option() -> StringBuilder {
    StringBuilder::new("telephone", "Numéro de téléphone (« - » pour l'effacer)").max_length(PHONE_LIMIT as u16)
}

fn description_option() -> StringBuilder {
    StringBuilder::new("description", "Description visible de tous (« - » pour l'effacer)")
        .max_length(DESCRIPTION_LIMIT as u16)
}

pub fn definition() -> Command {
    CommandBuilder::new("contact", "Répertoire des groupes, entreprises et contacts", CommandType::ChatInput)
        .option(SubCommandBuilder::new("voir", "Fiche d'un contact").option(contact_option("Le contact")))
        .option(
            SubCommandBuilder::new("liste", "Tous les contacts, par relation")
                .option(relation_option("Seulement cette relation"))
                .option(kind_option("Seulement ce type")),
        )
        .option(
            SubCommandBuilder::new("ajouter", "Ajoute un contact")
                .option(
                    StringBuilder::new("nom", "Nom du groupe, de l'entreprise ou de la personne")
                        .required(true)
                        .max_length(NAME_LIMIT as u16),
                )
                .option(kind_option("Groupe, entreprise ou personne").required(true))
                .option(relation_option("Relation avec le groupe").required(true))
                .option(phone_option())
                .option(description_option()),
        )
        .option(
            SubCommandBuilder::new("modifier", "Modifie un contact")
                .option(contact_option("Le contact à modifier"))
                .option(StringBuilder::new("nom", "Nouveau nom").max_length(NAME_LIMIT as u16))
                .option(kind_option("Nouveau type"))
                .option(relation_option("Nouvelle relation"))
                .option(phone_option())
                .option(description_option()),
        )
        .option(
            SubCommandBuilder::new("supprimer", "Supprime un contact et ses notes")
                .option(contact_option("Le contact à supprimer")),
        )
        .option(
            SubCommandBuilder::new("note", "Ajoute une note réservée au staff")
                .option(contact_option("Le contact"))
                .option(StringBuilder::new("texte", "La note").required(true).max_length(NOTE_LIMIT as u16)),
        )
        .build()
}

/// `/config contact`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("contact", "Contacts : salon du répertoire permanent (sans option : affiche)")
        .option(
            ChannelBuilder::new("salon", "Salon où publier le répertoire des contacts")
                .channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("sans_annuaire", "Retirer le répertoire publié dans le salon"))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let channel = options.channel("salon").map(|id| id.to_string());
    let current = service::get_config(&ix.ctx.db, guild).await?;
    if channel.is_none() && options.boolean("sans_annuaire") != Some(true) {
        return ix
            .reply(Reply::ephemeral(match current.channel_id {
                Some(channel) => format!("Le répertoire des contacts est publié dans <#{channel}>."),
                None => "Le répertoire des contacts n'est publié dans aucun salon (`/contact liste` reste disponible)."
                    .to_owned(),
            }))
            .await;
    }
    ix.defer(true).await?;
    // L'ancien message disparaît : changement de salon ou retrait de l'annuaire.
    if let (Some(old_channel), Some(old_message)) =
        (current.channel_id.as_deref().and_then(parse_id), current.message_id.as_deref().and_then(parse_id))
    {
        messages::delete_quietly(&ix.ctx, old_channel, old_message).await;
    }
    service::set_channel(&ix.ctx.db, guild, channel.as_deref()).await?;
    match channel {
        Some(channel) => {
            service::sync_directory(&ix.ctx, guild).await?;
            ix.edit_text(format!("Répertoire des contacts publié dans <#{channel}>.")).await
        }
        None => ix.edit_text("Le répertoire des contacts n'est plus publié.").await,
    }
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    match options.sub() {
        "voir" => show(ix, options, guild_id).await,
        "liste" => list(ix, options, guild_id).await,
        "ajouter" => add(ix, options, guild_id).await,
        "modifier" => update(ix, options, guild_id).await,
        "supprimer" => remove(ix, options, guild_id).await,
        "note" => note(ix, options, guild_id).await,
        _ => Ok(()),
    }
}

/// Met à jour l'annuaire du salon sans faire échouer l'action déjà faite.
async fn refresh_directory(ix: &Ix, guild: &str) {
    if let Err(error) = service::sync_directory(&ix.ctx, guild).await {
        tracing::warn!(?error, "mise à jour du répertoire des contacts impossible");
    }
}

/// Le contact de l'option `contact` (identifiant de l'autocomplétion, ou nom).
async fn target(ix: &Ix, options: &Options, guild: &str) -> Result<Option<Contact>> {
    service::resolve(&ix.ctx.db, guild, &options.string("contact").unwrap_or_default()).await
}

fn optional(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("—")
}

async fn show(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactVoir).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let Some(contact) = target(ix, options, &guild).await? else {
        return ix.reply(Reply::ephemeral(NOT_FOUND)).await;
    };
    let (kind, relation) = (contact.kind(), contact.relation());
    let mut description = contact.description.clone().unwrap_or_else(|| "*Pas de description.*".to_owned());
    if permissions::can(ix, Perm::ContactNotes).await? {
        let (notes, total) = service::notes(&ix.ctx.db, &guild, &contact.id, NOTES_SHOWN).await?;
        let header = format!("\n\n**🔒 Notes du staff ({total})**\n");
        let body = if notes.is_empty() {
            "*Aucune note. Ajoute-en une avec `/contact note`.*".to_owned()
        } else {
            let lines: Vec<String> = notes
                .iter()
                .map(|n| {
                    format!("• {} — <@{}> : {}", format_day_label(n.created_at), n.author_id, truncate(&n.body, 400))
                })
                .collect();
            let budget = fmt::DESCRIPTION_LIMIT.saturating_sub(description.len() + header.len() + 100);
            fmt::bounded_lines(&lines, budget, |hidden| format!("… et {hidden} note(s) de plus sur le site."))
        };
        description.push_str(&header);
        description.push_str(&body);
    }
    let embed = EmbedBuilder::new()
        .title(truncate(&format!("{} {}", kind.emoji(), contact.name), 256))
        .color(relation.color())
        .description(truncate(&description, fmt::DESCRIPTION_LIMIT))
        .field(field("Type", kind.label(), true))
        .field(field("Relation", relation.display(), true))
        .field(field("Téléphone", optional(&contact.phone), true))
        .footer(footer(format!("Mis à jour le {}", format_day_label(contact.updated_at))))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn list(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactVoir).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let filter = Filter {
        relation: options.string("relation").as_deref().and_then(Relation::parse),
        kind: options.string("type").as_deref().and_then(Kind::parse),
        query: None,
    };
    let contacts = service::list(&ix.ctx.db, &guild, &filter).await?;
    let description = if contacts.is_empty() {
        "Aucun contact.".to_owned()
    } else {
        let blocks: Vec<ListBlock> = Relation::ALL
            .iter()
            .filter(|relation| filter.relation.is_none_or(|r| r == **relation))
            .map(|relation| {
                let lines: Vec<String> =
                    contacts.iter().filter(|c| c.relation() == *relation).map(service::directory_line).collect();
                ListBlock {
                    header: format!("**{} {} ({})**", relation.emoji(), relation.plural_label(), lines.len()),
                    lines,
                    empty: "*Aucun.*".to_owned(),
                }
            })
            .collect();
        pack_blocks(&blocks, fmt::DESCRIPTION_LIMIT - 100).join("\n\n")
    };
    let mut footer_text = format!("{} contact{}", contacts.len(), fmt::plural(contacts.len()));
    if let Some(kind) = filter.kind {
        footer_text.push_str(&format!(" · {}", kind.label()));
    }
    if permissions::can(ix, Perm::ContactNotes).await?
        && let Some(url) = crate::web::auth::public_url(&ix.ctx)
    {
        footer_text.push_str(&format!(" · notes : {url}/stats/{guild}/contacts"));
    }
    let embed = EmbedBuilder::new()
        .title("📇 Répertoire des contacts")
        .color(COLOR)
        .description(description)
        .footer(footer(footer_text))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn add(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactAjouter).await? {
        return Ok(());
    }
    let name = options.string("nom").unwrap_or_default().trim().to_owned();
    let (Some(kind), Some(relation)) = (
        options.string("type").as_deref().and_then(Kind::parse),
        options.string("relation").as_deref().and_then(Relation::parse),
    ) else {
        return ix.reply(Reply::ephemeral("Type ou relation invalide.")).await;
    };
    let phone = optional_text(options.string("telephone"));
    let description = optional_text(options.string("description"));
    if let Err(message) = validate(&name, phone.as_deref(), description.as_deref()) {
        return ix.reply(Reply::ephemeral(message)).await;
    }
    let guild = guild_id.to_string();
    let new = NewContact { name, kind, relation, phone, description, created_by_id: ix.user_id().to_string() };
    let Some(contact) = service::add(&ix.ctx.db, &guild, new).await? else {
        return ix.reply(Reply::ephemeral(NAME_TAKEN)).await;
    };
    ix.defer(true).await?;
    refresh_directory(ix, &guild).await;
    audit::record(
        ix,
        Entry::new(
            Action::ContactAdd,
            None,
            format!("**{}** — {} · {}", contact.name, kind.label(), relation.display()),
        ),
    )
    .await;
    ix.edit_text(format!("Contact **{}** ajouté ({} · {}).", contact.name, kind.label(), relation.display())).await
}

/// Changements lisibles pour le journal : « relation : 🟢 Allié → 🔴 Hostile ».
fn changes(before: &Contact, after: &Contact) -> Vec<String> {
    let mut changes = Vec::new();
    if before.name != after.name {
        changes.push(format!("nom : {} → {}", before.name, after.name));
    }
    if before.kind != after.kind {
        changes.push(format!("type : {} → {}", before.kind().label(), after.kind().label()));
    }
    if before.relation != after.relation {
        changes.push(format!("relation : {} → {}", before.relation().display(), after.relation().display()));
    }
    if before.phone != after.phone {
        changes.push(format!("téléphone : {} → {}", optional(&before.phone), optional(&after.phone)));
    }
    if before.description != after.description {
        changes.push(match &after.description {
            Some(description) => format!("description : {}", truncate(description, 200)),
            None => "description effacée".to_owned(),
        });
    }
    changes
}

async fn update(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactModifier).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let Some(before) = target(ix, options, &guild).await? else {
        return ix.reply(Reply::ephemeral(NOT_FOUND)).await;
    };
    let mut after = before.clone();
    if let Some(name) = options.string("nom") {
        after.name = name.trim().to_owned();
    }
    if let Some(kind) = options.string("type").as_deref().and_then(Kind::parse) {
        after.kind = kind.as_str().to_owned();
    }
    if let Some(relation) = options.string("relation").as_deref().and_then(Relation::parse) {
        after.relation = relation.as_str().to_owned();
    }
    // Option absente : inchangé ; « - » : effacé.
    if let Some(phone) = options.string("telephone") {
        after.phone = optional_text(Some(phone));
    }
    if let Some(description) = options.string("description") {
        after.description = optional_text(Some(description));
    }
    if let Err(message) = validate(&after.name, after.phone.as_deref(), after.description.as_deref()) {
        return ix.reply(Reply::ephemeral(message)).await;
    }
    let changes = changes(&before, &after);
    if changes.is_empty() {
        return ix.reply(Reply::ephemeral("Rien à modifier : indique au moins un nouveau champ.")).await;
    }
    if !service::save(&ix.ctx.db, &guild, &after).await? {
        return ix.reply(Reply::ephemeral(NAME_TAKEN)).await;
    }
    ix.defer(true).await?;
    refresh_directory(ix, &guild).await;
    let summary = changes.join(" · ");
    audit::record(ix, Entry::new(Action::ContactUpdate, None, format!("**{}** — {summary}", after.name))).await;
    ix.edit_text(format!("Contact **{}** modifié : {summary}.", after.name)).await
}

async fn remove(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactModifier).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let Some(contact) = target(ix, options, &guild).await? else {
        return ix.reply(Reply::ephemeral(NOT_FOUND)).await;
    };
    let (_, notes) = service::notes(&ix.ctx.db, &guild, &contact.id, 0).await?;
    service::delete(&ix.ctx.db, &guild, &contact.id).await?;
    ix.defer(true).await?;
    refresh_directory(ix, &guild).await;
    audit::record(
        ix,
        Entry::new(
            Action::ContactDelete,
            None,
            format!(
                "**{}** — {} · {} · {notes} note{} supprimée{}",
                contact.name,
                contact.kind().label(),
                contact.relation().display(),
                fmt::plural(notes as usize),
                fmt::plural(notes as usize)
            ),
        ),
    )
    .await;
    ix.edit_text(format!("Contact **{}** supprimé, avec ses notes.", contact.name)).await
}

async fn note(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::ContactNotes).await? {
        return Ok(());
    }
    let guild = guild_id.to_string();
    let Some(contact) = target(ix, options, &guild).await? else {
        return ix.reply(Reply::ephemeral(NOT_FOUND)).await;
    };
    let body = options.string("texte").unwrap_or_default().trim().to_owned();
    if let Err(message) = validate_note(&body) {
        return ix.reply(Reply::ephemeral(message)).await;
    }
    service::add_note(&ix.ctx.db, &guild, &contact.id, &ix.user_id().to_string(), &body).await?;
    // Le salon du journal est réservé au staff : la note y figure, tronquée.
    audit::record(
        ix,
        Entry::new(Action::ContactNote, None, format!("**{}** — {}", contact.name, truncate(&body, 300))),
    )
    .await;
    ix.reply(Reply::ephemeral(format!(
        "Note ajoutée sur **{}**. Elle n'est visible que des gradés et leads (`/contact voir`, site).",
        contact.name
    )))
    .await
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    let query = options.focused.as_ref().map(|(_, value)| value.clone()).filter(|v| !v.trim().is_empty());
    let filter = Filter { query, ..Default::default() };
    let contacts = service::list(&ix.ctx.db, &guild_id.to_string(), &filter).await?;
    let choices = contacts
        .iter()
        .take(25)
        .map(|c| choice(&format!("{} {} · {}", c.relation().emoji(), c.name, c.kind().label()), c.id.clone()))
        .collect();
    ix.autocomplete(choices).await
}
