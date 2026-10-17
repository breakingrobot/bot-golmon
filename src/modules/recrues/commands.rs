use super::{
    COLOR, NOT_GRADE, NOT_LEAD, VETO_COLOR,
    channels::{EnsureChannel, ensure_channel},
    embed::{format_entry_detail, format_tally},
    is_grade, is_lead,
    service::{self, ConfigUpdate, RecrueConfig},
};
use crate::{
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply},
        options::Options,
    },
    util::{
        format::{field, mention, truncate_field},
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
    ChannelBuilder, CommandBuilder, RoleBuilder, StringBuilder, SubCommandBuilder, SubCommandGroupBuilder, UserBuilder,
};

fn member_option(description: &str) -> UserBuilder {
    UserBuilder::new("membre", description).required(true)
}

pub fn definition() -> Command {
    CommandBuilder::new("recrue", "Gestion des recrues en période d'essai", CommandType::ChatInput)
        .option(SubCommandBuilder::new("liste", "Affiche ou reposte la liste des avis de la semaine"))
        .option(
            SubCommandBuilder::new("avis", "Affiche le détail complet des avis pour un membre")
                .option(member_option("La recrue ciblée")),
        )
        .option(
            SubCommandBuilder::new("ajouter", "Ajoute manuellement un membre à la liste de la semaine")
                .option(member_option("Le membre à ajouter")),
        )
        .option(
            SubCommandBuilder::new("retirer", "Retire manuellement un membre de la liste de la semaine")
                .option(member_option("Le membre à retirer")),
        )
        .option(
            SubCommandBuilder::new("salon", "Crée manuellement le salon privé d'une recrue")
                .option(member_option("La recrue ciblée")),
        )
        .option(
            SubCommandBuilder::new("historique", "Consulte l'historique des avis d'un membre")
                .option(member_option("Le membre ciblé")),
        )
        .option(
            SubCommandGroupBuilder::new("veto", "Gestion des vétos").subcommands([
                SubCommandBuilder::new("poser", "Pose un véto sur une recrue")
                    .option(member_option("La recrue ciblée"))
                    .option(StringBuilder::new("motif", "Motif du véto").required(true)),
                SubCommandBuilder::new("lever", "Lève le véto d'une recrue").option(member_option("La recrue ciblée")),
            ]),
        )
        .build()
}

/// `/config recrue`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("recrue", "Recrues : rôles, catégorie et salon des avis (sans option : affiche)")
        .option(RoleBuilder::new("role_recrue", "Le rôle attribué aux membres en essai"))
        .option(RoleBuilder::new("role_grade", "Le rôle autorisé à donner son avis"))
        .option(RoleBuilder::new("role_lead", "Le rôle autorisé à poser un véto"))
        .option(
            ChannelBuilder::new("categorie", "La catégorie où créer les salons privés")
                .channel_types([ChannelType::GuildCategory]),
        )
        .option(
            ChannelBuilder::new("salon", "Le salon où publier les avis de la semaine")
                .channel_types([ChannelType::GuildText]),
        )
}

/// `/gestion recrue reset`.
pub fn admin_subcommands() -> Vec<SubCommandBuilder> {
    vec![SubCommandBuilder::new("reset", "Force le démarrage d'une nouvelle session hebdomadaire")]
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if options.group() == Some("veto") {
        return match options.sub() {
            "poser" => veto_poser(ix, options, guild_id).await,
            "lever" => veto_lever(ix, options, guild_id).await,
            _ => Ok(()),
        };
    }
    match options.sub() {
        "liste" => liste(ix, guild_id).await,
        "ajouter" => ajouter(ix, options, guild_id).await,
        "retirer" => retirer(ix, options, guild_id).await,
        "salon" => salon(ix, options, guild_id).await,
        "avis" => avis(ix, options, guild_id).await,
        "historique" => historique(ix, options, guild_id).await,
        _ => Ok(()),
    }
}

/// Garde commune aux sous-commandes réservées aux gradés.
async fn require_grade(ix: &Ix, guild_id: Id<GuildMarker>) -> Result<Option<Option<RecrueConfig>>> {
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    if !is_grade(ix, config.as_ref()) {
        ix.reply(Reply::ephemeral(NOT_GRADE)).await?;
        return Ok(None);
    }
    Ok(Some(config))
}

fn describe(id: Option<&String>, role: bool) -> String {
    match id {
        None => "*non configuré*".to_owned(),
        Some(id) if role => format!("<@&{id}>"),
        Some(id) => format!("<#{id}>"),
    }
}

pub async fn config(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let update = ConfigUpdate {
        recrue_role_id: options.role("role_recrue").map(|id| id.to_string()),
        grade_role_id: options.role("role_grade").map(|id| id.to_string()),
        lead_role_id: options.role("role_lead").map(|id| id.to_string()),
        category_id: options.channel("categorie").map(|id| id.to_string()),
        dashboard_channel_id: options.channel("salon").map(|id| id.to_string()),
    };

    // Sans aucune option, la commande sert à consulter la configuration.
    if update.is_empty() {
        let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
        let c = config.as_ref();
        let mut embed = EmbedBuilder::new()
            .title("Configuration des recrues")
            .color(COLOR)
            .field(field("Rôle recrue", describe(c.and_then(|c| c.recrue_role_id.as_ref()), true), false))
            .field(field("Rôle gradé (vote)", describe(c.and_then(|c| c.grade_role_id.as_ref()), true), false))
            .field(field("Rôle lead (véto)", describe(c.and_then(|c| c.lead_role_id.as_ref()), true), false))
            .field(field(
                "Catégorie des salons d'essai",
                describe(c.and_then(|c| c.category_id.as_ref()), false),
                false,
            ))
            .field(field("Salon des avis", describe(c.and_then(|c| c.dashboard_channel_id.as_ref()), false), false));
        if c.and_then(|c| c.recrue_role_id.as_ref()).is_none() || c.and_then(|c| c.grade_role_id.as_ref()).is_none() {
            embed = embed.description("⚠️ Les rôles recrue et gradé sont indispensables au fonctionnement du module.");
        }
        return ix.reply(Reply::embed(embed.build()).hidden()).await;
    }

    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    ix.defer(true).await?;
    service::upsert_config(&ix.ctx.db, &guild_id.to_string(), update).await?;
    service::ensure_current_session(&ix.ctx, guild_id).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text("Configuration mise à jour. `/config recrue` sans option pour la consulter.").await
}

async fn liste(ix: &Ix, guild_id: Id<GuildMarker>) -> Result<()> {
    let Some(config) = require_grade(ix, guild_id).await? else {
        return Ok(());
    };
    let Some(channel) = config.and_then(|c| c.dashboard_channel_id) else {
        return ix
            .reply(Reply::ephemeral("Aucun salon d'avis configuré. Utilise `/config recrue salon:#salon`."))
            .await;
    };
    ix.defer(true).await?;
    let session = service::ensure_current_session(&ix.ctx, guild_id).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text(format!("Liste des avis à jour dans <#{channel}> ({} recrue(s)).", session.entries.len())).await
}

async fn ajouter(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let membre = options.user("membre").expect("option requise");
    add_member(ix, options, guild_id, membre).await
}

/// Ajoute un membre aux recrues de la semaine (`/recrue ajouter`, clic droit « Ajouter aux recrues »).
pub async fn add_member(
    ix: &Ix,
    options: &Options,
    guild_id: Id<GuildMarker>,
    membre: Id<twilight_model::id::marker::UserMarker>,
) -> Result<()> {
    if require_grade(ix, guild_id).await?.is_none() {
        return Ok(());
    }
    let name = options.user_name(membre);
    ix.defer(true).await?;

    let session = service::ensure_current_session(&ix.ctx, guild_id).await?;
    service::add_entry(&ix.ctx.db, &session.id, &guild_id.to_string(), &membre.to_string()).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;

    let channel_note = match ensure_channel(&ix.ctx, guild_id, membre).await? {
        EnsureChannel::Ok { created, channel_id } => {
            format!(" Salon privé {} : <#{channel_id}>.", if created { "créé" } else { "existant" })
        }
        EnsureChannel::Failed(reason) => format!(" ⚠️ Salon non créé — {reason}"),
    };
    ix.edit_text(format!("{name} figure désormais dans la liste des recrues de la semaine.{channel_note}")).await
}

async fn retirer(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if require_grade(ix, guild_id).await?.is_none() {
        return Ok(());
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    ix.defer(true).await?;

    let session = service::ensure_current_session(&ix.ctx, guild_id).await?;
    let removed = service::remove_entry(&ix.ctx.db, &session.id, &membre.to_string()).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text(if removed {
        format!("{name} ne figure plus dans la liste de la semaine (son salon privé est conservé).")
    } else {
        format!("{name} ne figurait pas dans la liste de la semaine.")
    })
    .await
}

async fn salon(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if require_grade(ix, guild_id).await?.is_none() {
        return Ok(());
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    ix.defer(true).await?;
    match ensure_channel(&ix.ctx, guild_id, membre).await? {
        EnsureChannel::Failed(reason) => ix.edit_text(format!("Impossible de créer le salon — {reason}")).await,
        EnsureChannel::Ok { created: true, channel_id } => {
            ix.edit_text(format!("Salon <#{channel_id}> créé pour {name}.")).await
        }
        EnsureChannel::Ok { created: false, channel_id } => {
            ix.edit_text(format!("{name} a déjà un salon : <#{channel_id}>.")).await
        }
    }
}

async fn avis(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if require_grade(ix, guild_id).await?.is_none() {
        return Ok(());
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    let Some((entry, session)) =
        service::find_entry_in_current_session(&ix.ctx.db, &guild_id.to_string(), &membre.to_string()).await?
    else {
        return ix
            .reply(Reply::ephemeral(format!("{name} ne figure pas dans la liste des recrues de la semaine.")))
            .await;
    };
    let embed = EmbedBuilder::new()
        .title(format!("Avis sur {name}"))
        .color(if entry.veto_by_id.is_some() { VETO_COLOR } else { COLOR })
        .description(format!("{}\n{}", mention(&entry.user_id), format_tally(&entry.votes)))
        .field(field("Détail", format_entry_detail(&entry), false))
        .footer(footer(format!("Semaine du {}", format_day_label(session.week_start))))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn historique(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if require_grade(ix, guild_id).await?.is_none() {
        return Ok(());
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    let history = service::user_history(&ix.ctx.db, &guild_id.to_string(), &membre.to_string()).await?;
    if history.is_empty() {
        return ix.reply(Reply::ephemeral(format!("Aucun historique d'avis pour {name}."))).await;
    }
    let mut embed = EmbedBuilder::new().title(format!("Historique des avis — {name}")).color(COLOR);
    for (entry, week) in &history {
        embed = embed.field(field(
            format!("Semaine du {}{}", format_day_label(*week), if entry.veto_by_id.is_some() { " ⛔" } else { "" }),
            truncate_field(&format!(
                "{}{}",
                format_tally(&entry.votes),
                entry.veto_motif.as_deref().map(|m| format!("\n⛔ {m}")).unwrap_or_default()
            )),
            false,
        ));
    }
    ix.reply(Reply::embed(embed.build()).hidden()).await
}

pub async fn reset(ix: &Ix, guild_id: Id<GuildMarker>) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    ix.defer(true).await?;
    let session = service::ensure_current_session(&ix.ctx, guild_id).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text(format!(
        "Semaine en cours : {} recrue(s). Les semaines précédentes restent consultables via `/recrue historique`.",
        session.entries.len()
    ))
    .await
}

async fn veto_poser(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    if !is_lead(ix, config.as_ref()) {
        return ix.reply(Reply::ephemeral(NOT_LEAD)).await;
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    let motif = options.string("motif").unwrap_or_default().trim().to_owned();
    let Some((entry, _)) =
        service::find_entry_in_current_session(&ix.ctx.db, &guild_id.to_string(), &membre.to_string()).await?
    else {
        return ix
            .reply(Reply::ephemeral(format!("{name} ne figure pas dans la liste des recrues de la semaine.")))
            .await;
    };
    ix.defer(true).await?;
    service::set_veto(&ix.ctx.db, &entry.id, &ix.user_id().to_string(), &motif).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text(format!("⛔ Véto posé sur {name}.")).await
}

async fn veto_lever(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    if !is_lead(ix, config.as_ref()) {
        return ix.reply(Reply::ephemeral(NOT_LEAD)).await;
    }
    let membre = options.user("membre").expect("option requise");
    let name = options.user_name(membre);
    let entry = service::find_entry_in_current_session(&ix.ctx.db, &guild_id.to_string(), &membre.to_string()).await?;
    let Some((entry, _)) = entry.filter(|(e, _)| e.veto_by_id.is_some()) else {
        return ix.reply(Reply::ephemeral(format!("Aucun véto actif sur {name}."))).await;
    };
    ix.defer(true).await?;
    service::clear_veto(&ix.ctx.db, &entry.id).await?;
    service::sync_dashboard(&ix.ctx, guild_id).await?;
    ix.edit_text(format!("Véto levé sur {name}.")).await
}
