use super::{
    absences::{self, NewAbsence},
    blocklist,
    model::ResponseStatus,
    service::{self, ConfigUpdate, NewPresence},
};
use crate::{
    discord::{
        embed::{EmbedBuilder, EmbedBuilderExt as _, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        members::avatar_url,
        messages::{self, Mentions, Outgoing, parse_id},
        options::Options,
    },
    util::{
        format::{DESCRIPTION_LIMIT, chunk_mentions, field, truncate, truncate_field},
        time::{self, DbTime, format_date_label, parse_event_date, today_label},
    },
};
use anyhow::Result;
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
    id::{Id, marker::UserMarker},
};
use twilight_util::builder::command::{
    ChannelBuilder, CommandBuilder, IntegerBuilder, RoleBuilder, StringBuilder, SubCommandBuilder, UserBuilder,
};

fn presence_option(description: &str, required: bool) -> StringBuilder {
    StringBuilder::new("presence", description).required(required).autocomplete(true)
}

pub fn presence_definition() -> Command {
    CommandBuilder::new("presence", "Gestion des présences de groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("create", "Crée une présence (rôles et salon viennent de la config)")
                .option(StringBuilder::new("heure", "Heure, ex: 21:00").required(true))
                .option(StringBuilder::new("titre", "Titre (défaut: Présence)"))
                .option(StringBuilder::new("date", "Date JJ/MM/AAAA (défaut: aujourd'hui)"))
                .option(StringBuilder::new("description", "Description")),
        )
        .option(
            SubCommandBuilder::new("stats", "Statistiques de présence")
                .option(UserBuilder::new("joueur", "Joueur (défaut: classement du serveur)")),
        )
        .option(
            SubCommandBuilder::new("ping", "Relance les membres n'ayant pas répondu")
                .option(presence_option("Présence concernée", true)),
        )
        .option(
            SubCommandBuilder::new("close", "Clôture une présence").option(presence_option("Présence concernée", true)),
        )
        .option(
            SubCommandBuilder::new("changes", "Affiche l'historique des changements pour une présence")
                .option(presence_option("Présence concernée", true)),
        )
        .build()
}

/// `/config presence`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("presence", "Présences : rôles et salons (sans option : affiche la config)")
        .option(RoleBuilder::new("role", "Rôle ciblé par les présences"))
        .option(RoleBuilder::new("absence_role", "Rôle donné pendant une absence (auto-absent, non ping)"))
        .option(ChannelBuilder::new("channel", "Salon où poster les présences").channel_types([ChannelType::GuildText]))
        .option(
            ChannelBuilder::new("reminder_channel", "Salon des rappels automatiques")
                .channel_types([ChannelType::GuildText]),
        )
        .option(
            ChannelBuilder::new("absence_channel", "Salon où annoncer les absences")
                .channel_types([ChannelType::GuildText]),
        )
        .option(
            ChannelBuilder::new("rapport_salon", "Salon du rapport d'assiduité du lundi (sans salon : pas de rapport)")
                .channel_types([ChannelType::GuildText]),
        )
        .option(
            IntegerBuilder::new("seuil_alerte", "Rapport : taux de présence sur 4 semaines sous lequel alerter (%)")
                .min_value(1)
                .max_value(100),
        )
}

/// Sous-commandes de `/gestion presence` (staff).
pub fn admin_subcommands() -> Vec<SubCommandBuilder> {
    vec![
        SubCommandBuilder::new("forcevote", "Définit manuellement la réponse d'un membre")
            .option(UserBuilder::new("utilisateur", "Membre concerné").required(true))
            .option(
                StringBuilder::new("statut", "Statut à appliquer")
                    .required(true)
                    .choices(ResponseStatus::ALL.map(|s| (s.label(), s.as_str()))),
            )
            .option(presence_option("Présence visée (défaut: la plus proche)", false))
            .option(IntegerBuilder::new("retard", "Durée du retard en minutes").min_value(0)),
        SubCommandBuilder::new("annuler", "Annule et supprime complètement une présence")
            .option(presence_option("Présence à annuler", true)),
        SubCommandBuilder::new("reset", "Réinitialise toutes les statistiques de présence (« Gérer le serveur »)"),
    ]
    .into_iter()
    .chain(super::planning::admin_subcommands())
    .collect()
}

/// Sous-commandes de `/gestion blocklist` (staff).
pub fn blocklist_subcommands() -> Vec<SubCommandBuilder> {
    vec![
        SubCommandBuilder::new("ajouter", "Ajoute un membre à la blocklist (jamais pingé, absent d'office)")
            .option(UserBuilder::new("membre", "Membre concerné").required(true))
            .option(StringBuilder::new("raison", "Raison")),
        SubCommandBuilder::new("retirer", "Retire un membre de la blocklist")
            .option(UserBuilder::new("membre", "Membre concerné").required(true)),
        SubCommandBuilder::new("liste", "Affiche la blocklist"),
    ]
}

/// `/gestion presence …` et `/gestion blocklist …`.
pub async fn handle_admin(ix: &Ix, options: &Options) -> Result<()> {
    if options.group() == Some("blocklist") {
        return blocklist_command(ix, options).await;
    }
    match options.sub() {
        "forcevote" => force_vote(ix, options).await,
        "annuler" => cancel(ix, options).await,
        "reset" => reset(ix).await,
        "planifier" | "planning" | "deplanifier" => super::planning::handle_admin(ix, options).await,
        _ => Ok(()),
    }
}

pub fn absence_definition() -> Command {
    CommandBuilder::new("absence", "Signaler une absence", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("declare", "Signale ton absence (le rôle d'absence est appliqué automatiquement)")
                .option(StringBuilder::new("motif", "Motif de l'absence").required(true))
                .option(StringBuilder::new("fin", "Dernier jour d'absence, JJ/MM/AAAA").required(true))
                .option(StringBuilder::new("debut", "Premier jour, JJ/MM/AAAA (défaut: aujourd'hui)"))
                .option(UserBuilder::new("membre", "Déclarer pour un autre membre (staff)")),
        )
        .option(
            SubCommandBuilder::new("end", "Termine ton absence en cours, ou annule ta prochaine absence")
                .option(UserBuilder::new("membre", "Membre concerné (staff)")),
        )
        .option(SubCommandBuilder::new("list", "Liste les absences en cours"))
        .build()
}

pub async fn handle_presence(ix: &Ix, options: &Options) -> Result<()> {
    match options.sub() {
        "create" => create(ix, options).await,
        "stats" => stats(ix, options).await,
        "ping" => ping(ix, options).await,
        "close" => close(ix, options).await,
        "changes" => changes(ix, options).await,
        _ => Ok(()),
    }
}

pub async fn handle_absence(ix: &Ix, options: &Options) -> Result<()> {
    match options.sub() {
        "declare" => absence_declare(ix, options).await,
        "end" => absence_end(ix, options).await,
        "list" => absence_list(ix).await,
        _ => Ok(()),
    }
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    let query = options.focused.as_ref().map(|(_, value)| value.as_str()).unwrap_or_default();
    // forcevote sert aussi à corriger après coup : il doit voir les clôturées.
    let include_closed = options.sub() == "forcevote";
    let presences = service::find_presences(&ix.ctx.db, &guild_id.to_string(), query, include_closed).await?;
    let choices = presences
        .iter()
        .map(|p| {
            let lock = if p.closed_at.is_some() { "🔒 " } else { "" };
            choice(&format!("{lock}{} — {}", p.titre, format_date_label(p.event_date)), p.id.clone())
        })
        .collect();
    ix.autocomplete(choices).await
}

async fn create(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    let Some(config) = config.filter(|c| c.default_role_id.is_some()) else {
        return ix
            .reply(Reply::ephemeral(
                "Aucun rôle configuré. Un admin doit d'abord lancer `/config presence role:@TonRôle` (une seule fois).",
            ))
            .await;
    };
    let role_id = config.default_role_id.clone().unwrap_or_default();

    let heure = options.string("heure").unwrap_or_default();
    let Some(resolved) = time::resolve_event_date(options.string("date").as_deref(), &heure) else {
        return ix.reply(Reply::ephemeral("Date ou heure invalide. Formats attendus : `JJ/MM/AAAA` et `HH:MM`.")).await;
    };

    let channel_id = config.presence_channel_id.as_deref().and_then(parse_id).or(ix.channel_id());
    let Some(channel_id) = channel_id else {
        return ix
            .reply(Reply::ephemeral("Salon des présences introuvable ou inaccessible. Vérifie `/config presence`."))
            .await;
    };

    ix.defer(true).await?;

    let published = service::publish_presence(
        &ix.ctx,
        guild_id,
        NewPresence {
            channel_id: channel_id.to_string(),
            titre: options.string("titre").unwrap_or_else(|| "Présence".to_owned()),
            description: options.string("description"),
            event_date: resolved.date,
            role_id,
            absence_role_id: config.absence_role_id.clone(),
            created_by_id: ix.user_id().to_string(),
        },
    )
    .await?;
    if published.is_none() {
        return ix
            .edit_text("La présence n'a pas pu être publiée (Discord a refusé la requête ou le salon est inaccessible). Réessaie dans un instant.")
            .await;
    }

    let rolled = if resolved.rolled_to_tomorrow { " (heure déjà passée aujourd'hui → **demain**)" } else { "" };
    ix.edit_text(format!("Présence créée pour <t:{}:F>{rolled} dans <#{channel_id}>.", resolved.date.unix())).await
}

fn describe(id: Option<&str>, kind: char) -> String {
    match id {
        None => "*non configuré*".to_owned(),
        Some(id) if kind == 'r' => format!("<@&{id}>"),
        Some(id) => format!("<#{id}>"),
    }
}

pub async fn config(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    // Configuration réservée aux administrateurs, lecture comprise.
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }

    let update = ConfigUpdate {
        default_role_id: options.role("role").map(|id| id.to_string()),
        absence_role_id: options.role("absence_role").map(|id| id.to_string()),
        presence_channel_id: options.channel("channel").map(|id| id.to_string()),
        reminder_channel_id: options.channel("reminder_channel").map(|id| id.to_string()),
        absence_channel_id: options.channel("absence_channel").map(|id| id.to_string()),
        report_channel_id: options.channel("rapport_salon").map(|id| id.to_string()),
        alert_threshold: options.integer("seuil_alerte").map(|value| value.clamp(1, 100)),
    };

    // Sans aucune option, la commande sert à consulter la configuration.
    if update.is_empty() {
        let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
        let get =
            |f: fn(&super::model::PresenceConfig) -> Option<&String>| config.as_ref().and_then(f).map(String::as_str);
        let mut embed = EmbedBuilder::new()
            .title("Configuration des présences")
            .color(0x5865f2)
            .field(field("Rôle ciblé", describe(get(|c| c.default_role_id.as_ref()), 'r'), false))
            .field(field("Rôle d'absence", describe(get(|c| c.absence_role_id.as_ref()), 'r'), false))
            .field(field("Salon des présences", describe(get(|c| c.presence_channel_id.as_ref()), 'c'), false))
            .field(field("Salon des rappels", describe(get(|c| c.reminder_channel_id.as_ref()), 'c'), false))
            .field(field("Salon des absences", describe(get(|c| c.absence_channel_id.as_ref()), 'c'), false))
            .field(field(
                "Rapport d'assiduité (lundi 09:00)",
                match config.as_ref().and_then(|c| c.last_report_at.filter(|_| c.report_channel_id.is_some())) {
                    Some(last) => format!(
                        "{} · dernier envoi <t:{}:R>",
                        describe(get(|c| c.report_channel_id.as_ref()), 'c'),
                        last.unix()
                    ),
                    None => describe(get(|c| c.report_channel_id.as_ref()), 'c'),
                },
                false,
            ))
            .field(field(
                "Seuil d'alerte du rapport",
                format!(
                    "{} % sur 4 semaines",
                    config.as_ref().map_or(super::model::DEFAULT_ALERT_THRESHOLD, |c| c.alert_threshold())
                ),
                false,
            ));
        if get(|c| c.default_role_id.as_ref()).is_none() {
            embed = embed.description("⚠️ Le rôle ciblé est obligatoire pour créer des présences.");
        }
        return ix.reply(Reply::embed(embed.build()).hidden()).await;
    }

    service::upsert_config(&ix.ctx.db, &guild_id.to_string(), update).await?;
    ix.reply(Reply::ephemeral("Configuration mise à jour. `/config presence` sans option pour la consulter.")).await
}

/// Fiche de présence d'un membre (`/presence stats joueur`, clic droit « Présences du membre »).
pub async fn member_stats(ix: &Ix, options: &Options, target: Id<UserMarker>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let stats = service::user_stats(&ix.ctx.db, &guild_id.to_string(), &target.to_string()).await?;

    let changes = if stats.recent_changes.is_empty() {
        "Aucun changement".to_owned()
    } else {
        stats
            .recent_changes
            .iter()
            .map(|change| {
                let from = change
                    .from_status
                    .as_deref()
                    .and_then(ResponseStatus::parse)
                    .map_or_else(|| "*aucune réponse*".to_owned(), ResponseStatus::display);
                let to = ResponseStatus::parse(&change.to_status).map(ResponseStatus::display).unwrap_or_default();
                format!("• **{}** : {from} → {to}", change.titre)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let name = options.user_name(target);
    let embed = EmbedBuilder::new()
        .title(format!("Statistiques de {name}"))
        .color(0x5865f2)
        .field(field("Présences clôturées", stats.total_closed.to_string(), true))
        .field(field("% de présence", format!("{}%", stats.percent), true))
        .field(field("\u{200b}", "\u{200b}", true))
        .field(field("✅ Présent", stats.present.to_string(), true))
        .field(field("❌ Absent", stats.absent.to_string(), true))
        .field(field("⏳ Retard", stats.retard.to_string(), true))
        .field(field("🕓 Sans réponse", stats.sans_reponse.to_string(), true))
        .field(field("Derniers changements d'avis", truncate_field(&changes), false))
        .maybe_thumbnail(options.resolved_user(target).map(|u| avatar_url(&u)))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn stats(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };

    if let Some(target) = options.user("joueur") {
        return member_stats(ix, options, target).await;
    }

    let leaderboard = service::leaderboard(&ix.ctx.db, &guild_id.to_string()).await?;
    if leaderboard.is_empty() {
        return ix.reply(Reply::ephemeral("Aucune donnée de présence pour l'instant.")).await;
    }
    let lines = leaderboard
        .iter()
        .take(20)
        .enumerate()
        .map(|(index, e)| {
            format!(
                "{}. <@{}> — **{}%** (✅{} ❌{} ⏳{})",
                index + 1,
                e.user_id,
                e.percent,
                e.present,
                e.absent,
                e.retard
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let embed = EmbedBuilder::new()
        .title("Classement de présence")
        .color(0x5865f2)
        .description(truncate(&lines, DESCRIPTION_LIMIT))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn ping(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let presence_id = options.string("presence").unwrap_or_default();
    let Some(presence) = service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), &presence_id).await? else {
        return ix.reply(Reply::ephemeral("Présence introuvable.")).await;
    };

    ix.defer(true).await?;
    let ids = service::non_respondent_ids(&ix.ctx, &presence).await?;
    if ids.is_empty() {
        return ix.edit_text("Tout le monde a répondu.").await;
    }
    let Some(channel_id) = parse_id(&presence.channel_id) else {
        return ix.edit_text("Salon de la présence inaccessible.").await;
    };
    for content in chunk_mentions(&ids, &format!("⏰ Rappel — **{}**", presence.titre)) {
        if messages::send(&ix.ctx, channel_id, &Outgoing::text(content).with_mentions(Mentions::Users)).await.is_err() {
            return ix.edit_text("Salon de la présence inaccessible.").await;
        }
    }
    ix.edit_text(format!("{} membre(s) relancé(s).", ids.len())).await
}

async fn force_vote(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix.reply(Reply::ephemeral("Réservé aux membres pouvant gérer les messages.")).await;
    }

    let target = options.user("utilisateur").expect("option requise");
    let Some(status) = options.string("statut").as_deref().and_then(ResponseStatus::parse) else {
        return ix.reply(Reply::ephemeral("Statut inconnu.")).await;
    };
    let retard = options.integer("retard");

    // Sans présence précisée, on vise la présence ouverte la plus proche.
    let presence = match options.string("presence") {
        Some(id) => service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), &id).await?,
        None => service::current_presence(&ix.ctx.db, &guild_id.to_string()).await?,
    };
    let Some(presence) = presence else {
        return ix.reply(Reply::ephemeral("Aucune présence trouvée. Précise-la avec l'option `presence`.")).await;
    };

    ix.defer(true).await?;
    let changed = service::register_response(&ix.ctx.db, &presence.id, &target.to_string(), status, retard).await?;
    if changed {
        service::sync_message(&ix.ctx, &presence.id).await?;
    }
    let suffix = if changed { "" } else { " (déjà le cas, rien changé)" };
    ix.edit_text(format!("<@{target}> défini sur **{}** pour « {} »{suffix}.", status.display(), presence.titre)).await
}

async fn close(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let presence_id = options.string("presence").unwrap_or_default();
    if service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), &presence_id).await?.is_none() {
        return ix.reply(Reply::ephemeral("Présence introuvable.")).await;
    }
    ix.defer(true).await?;
    service::close_presence(&ix.ctx.db, &presence_id).await?;
    service::sync_message(&ix.ctx, &presence_id).await?;
    ix.edit_text("Présence clôturée.").await
}

async fn cancel(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix
            .reply(Reply::ephemeral("Tu dois avoir la permission de gérer les messages pour annuler une présence."))
            .await;
    }
    let presence_id = options.string("presence").unwrap_or_default();
    let Some(presence) = service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), &presence_id).await? else {
        return ix.reply(Reply::ephemeral("Présence introuvable.")).await;
    };

    ix.defer(true).await?;
    if let (Some(channel_id), Some(message_id)) =
        (parse_id(&presence.channel_id), presence.message_id.as_deref().and_then(parse_id))
    {
        messages::delete_quietly(&ix.ctx, channel_id, message_id).await;
    }
    service::delete_presence(&ix.ctx.db, &presence.id).await?;
    ix.edit_text("Présence annulée et supprimée.").await
}

async fn changes(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let presence_id = options.string("presence").unwrap_or_default();
    let Some(presence) = service::get_guild_presence(&ix.ctx.db, &guild_id.to_string(), &presence_id).await? else {
        return ix.reply(Reply::ephemeral("Présence introuvable.")).await;
    };

    let changes = service::presence_changes(&ix.ctx.db, &presence.id).await?;
    if changes.is_empty() {
        return ix.reply(Reply::ephemeral("Aucun changement d'avis pour cette présence.")).await;
    }

    // Regroupe par membre, changements les plus récents d'abord.
    let mut lines: Vec<(String, Vec<String>)> = Vec::new();
    for change in &changes {
        let from = change
            .from_status
            .as_deref()
            .and_then(ResponseStatus::parse)
            .map(ResponseStatus::display)
            .unwrap_or_default();
        let to = ResponseStatus::parse(&change.to_status).map(ResponseStatus::display).unwrap_or_default();
        let entry = format!("{from} → {to} (<t:{}:R>)", change.changed_at.unix());
        match lines.iter_mut().find(|(user, _)| *user == change.user_id) {
            Some((_, entries)) => entries.push(entry),
            None => lines.push((change.user_id.clone(), vec![entry])),
        }
    }
    let text = lines
        .iter()
        .map(|(user, entries)| format!("• <@{user}> : {}", entries.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");

    let embed = EmbedBuilder::new()
        .title(truncate(&format!("Changements d'avis - {}", presence.titre), 256))
        .color(0x5865f2)
        .description(truncate(&text, DESCRIPTION_LIMIT))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn reset(ix: &Ix) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix
            .reply(Reply::ephemeral(
                "Seuls les membres avec la permission « Gérer le serveur » peuvent réinitialiser les statistiques.",
            ))
            .await;
    }
    ix.defer(true).await?;
    service::reset_guild(&ix.ctx.db, &guild_id.to_string()).await?;
    ix.edit_text("Toutes les statistiques et historiques de présences ont été réinitialisés avec succès.").await
}

async fn blocklist_command(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let guild = guild_id.to_string();

    if options.sub() == "liste" {
        let entries = blocklist::list(&ix.ctx.db, &guild).await?;
        let description = if entries.is_empty() {
            "Personne dans la blocklist.".to_owned()
        } else {
            truncate(
                &entries
                    .iter()
                    .map(|e| {
                        format!(
                            "• <@{}>{}",
                            e.user_id,
                            e.raison.as_deref().map(|r| format!(" — {r}")).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                DESCRIPTION_LIMIT,
            )
        };
        let embed = EmbedBuilder::new()
            .title("🚫 Blocklist présences")
            .color(0x95a5a6)
            .description(description)
            .footer(footer(format!("{} membre(s) jamais pingé(s)", entries.len())))
            .build();
        return ix.reply(Reply::embed(embed).hidden()).await;
    }

    // L'ajout et le retrait modifient qui reçoit les pings : réservés au staff.
    if !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix.reply(Reply::ephemeral("Réservé aux membres pouvant gérer les messages.")).await;
    }
    let member = options.user("membre").expect("option requise");

    match options.sub() {
        "ajouter" => {
            blocklist::add(
                &ix.ctx.db,
                &guild,
                &member.to_string(),
                options.string("raison").as_deref(),
                &ix.user_id().to_string(),
            )
            .await?;
            ix.reply(Reply::ephemeral(format!(
                "<@{member}> ne sera plus pingé pour les présences et sera marqué absent d'office."
            )))
            .await
        }
        "retirer" => {
            let removed = blocklist::remove(&ix.ctx.db, &guild, &member.to_string()).await?;
            ix.reply(Reply::ephemeral(if removed {
                format!("<@{member}> retiré de la blocklist.")
            } else {
                format!("<@{member}> n'était pas dans la blocklist.")
            }))
            .await
        }
        _ => Ok(()),
    }
}

/// Une date seule vaut le jour entier : début à 00:00, fin à 23:59.
fn day_boundary(label: &str, end: bool) -> Option<DbTime> {
    parse_event_date(label, if end { "23:59" } else { "00:00" })
}

async fn absence_declare(ix: &Ix, options: &Options) -> Result<()> {
    let target = options.user("membre").unwrap_or(ix.user_id());
    let avatar = options.resolved_user(target).or_else(|| ix.user().cloned()).map(|u| avatar_url(&u));
    declare_absence(
        ix,
        AbsenceForm {
            target,
            motif: options.string("motif").unwrap_or_default(),
            debut: options.string("debut"),
            fin: options.string("fin").unwrap_or_default(),
            avatar,
        },
    )
    .await
}

/// Absence saisie par commande ou par le formulaire du panneau.
pub(super) struct AbsenceForm {
    pub target: Id<UserMarker>,
    pub motif: String,
    /// `JJ/MM/AAAA`, aujourd'hui par défaut.
    pub debut: Option<String>,
    pub fin: String,
    pub avatar: Option<String>,
}

pub(super) async fn declare_absence(ix: &Ix, form: AbsenceForm) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let target = form.target;
    let for_someone_else = target != ix.user_id();
    if for_someone_else && !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix.reply(Reply::ephemeral("Seul le staff peut déclarer l'absence d'un autre membre.")).await;
    }

    let motif = form.motif.trim().to_owned();
    if motif.is_empty() {
        return ix.reply(Reply::ephemeral("Indique un motif.")).await;
    }
    let debut = form.debut.filter(|d| !d.trim().is_empty()).unwrap_or_else(today_label);
    let starts_at = day_boundary(debut.trim(), false);
    let ends_at = day_boundary(form.fin.trim(), true);
    let (Some(starts_at), Some(ends_at)) = (starts_at, ends_at) else {
        return ix.reply(Reply::ephemeral("Date invalide. Format attendu : `JJ/MM/AAAA`.")).await;
    };
    if ends_at <= starts_at {
        return ix.reply(Reply::ephemeral("La date de fin doit être postérieure à la date de début.")).await;
    }
    if ends_at <= DbTime::now() {
        return ix.reply(Reply::ephemeral("Cette absence est déjà terminée.")).await;
    }

    ix.defer(true).await?;
    let declared = absences::declare(
        &ix.ctx,
        guild_id,
        NewAbsence {
            user_id: target.to_string(),
            motif: motif.clone(),
            starts_at,
            ends_at,
            created_by_id: ix.user_id().to_string(),
        },
    )
    .await?;

    let mut embed = EmbedBuilder::new()
        .title("🌙 Absence déclarée")
        .color(0xe67e22)
        .field(field("Membre", format!("<@{target}>"), true))
        .field(field("Du", format!("<t:{}:d>", starts_at.unix()), true))
        .field(field("Au", format!("<t:{}:d>", ends_at.unix()), true))
        .field(field("Motif", truncate_field(&motif), false))
        .maybe_thumbnail(form.avatar);
    if for_someone_else {
        embed = embed.footer(footer(format!("Déclarée par {}", ix.username())));
    }
    let embed = embed.build();

    // L'annonce est un bonus : un salon inaccessible ne fait pas échouer la déclaration.
    // Elle est supprimée automatiquement quand l'absence se termine ou est annulée.
    let config = service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    if let Some(channel_id) = config.and_then(|c| c.absence_channel_id).as_deref().and_then(parse_id) {
        match messages::send(&ix.ctx, channel_id, &Outgoing::embeds(vec![embed.clone()])).await {
            Ok(message) => {
                absences::remember_announcement(
                    &ix.ctx.db,
                    &declared.id,
                    &channel_id.to_string(),
                    &message.id.to_string(),
                )
                .await?;
            }
            Err(error) => tracing::warn!(?error, "annonce d'absence impossible"),
        }
    }

    let presence_note = if declared.affected > 0 {
        format!("\n{} présence(s) ouverte(s) mise(s) à jour en absent.", declared.affected)
    } else {
        String::new()
    };
    let role_note = if declared.role.ok {
        String::new()
    } else {
        format!(
            "\n⚠️ Rôle d'absence non appliqué : {}.",
            declared.role.reason.unwrap_or_else(|| "raison inconnue".to_owned())
        )
    };
    ix.edit(
        Reply::text(format!("Absence enregistrée jusqu'au <t:{}:d>.{presence_note}{role_note}", ends_at.unix()))
            .with_embed(embed),
    )
    .await
}

async fn absence_end(ix: &Ix, options: &Options) -> Result<()> {
    let target = options.user("membre").unwrap_or(ix.user_id());
    end_absence(ix, target).await
}

/// Termine l'absence en cours du membre, sinon annule sa prochaine absence.
pub(super) async fn end_absence(ix: &Ix, target: Id<UserMarker>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if target != ix.user_id() && !ix.has_permission(Permissions::MANAGE_MESSAGES) {
        return ix.reply(Reply::ephemeral("Seul le staff peut clôturer l'absence d'un autre membre.")).await;
    }

    let guild = guild_id.to_string();
    let (absence, verb) = match absences::active_absence(&ix.ctx.db, &guild, &target.to_string()).await? {
        Some(absence) => (absence, "clôturée"),
        None => match absences::upcoming_absence(&ix.ctx.db, &guild, &target.to_string()).await? {
            Some(absence) => (absence, "annulée"),
            None => {
                return ix
                    .reply(Reply::ephemeral(format!("Aucune absence en cours ou à venir pour <@{target}>.")))
                    .await;
            }
        },
    };

    ix.defer(true).await?;
    let role = absences::end(&ix.ctx, guild_id, &absence.id, &target.to_string()).await?;
    let note = if role.ok {
        String::new()
    } else {
        format!("\n⚠️ Rôle non retiré : {}.", role.reason.unwrap_or_else(|| "raison inconnue".to_owned()))
    };
    ix.edit_text(format!("Absence de <@{target}> {verb}.{note}")).await
}

async fn absence_list(ix: &Ix) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let absences = absences::list_active(&ix.ctx.db, &guild_id.to_string()).await?;
    let description = if absences.is_empty() {
        "Personne n'est absent actuellement.".to_owned()
    } else {
        truncate(
            &absences
                .iter()
                .map(|a| format!("• <@{}> — jusqu'au <t:{}:d> — {}", a.user_id, a.ends_at.unix(), a.motif))
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let embed = EmbedBuilder::new()
        .title("🌙 Absences en cours")
        .color(0xe67e22)
        .description(description)
        .footer(footer(format!("{} absence(s)", absences.len())))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}
