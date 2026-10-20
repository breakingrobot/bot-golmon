use super::{
    dashboard, money,
    objective::{self, Counts, Objective},
    service::{self, Filter, NewCotisation, Totals},
};
use crate::{
    discord::{
        embed::{EmbedBuilder, EmbedBuilderExt as _, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply, choice},
        members::{avatar_url, display_name},
        messages::{self, Outgoing, parse_id},
        options::Options,
    },
    modules::{
        audit::{self, Action, Entry},
        permissions::{self, Perm},
    },
    util::{
        format::{DESCRIPTION_LIMIT, field, truncate, truncate_field},
        time::{DbTime, format_day_label, parse_event_date, today_label, week_start},
    },
};
use anyhow::Result;
use chrono::{Duration, Utc};
use twilight_model::{
    application::command::{Command, CommandType},
    channel::ChannelType,
    guild::Permissions,
    id::{Id, marker::GuildMarker},
};
use twilight_util::builder::command::{
    BooleanBuilder, ChannelBuilder, CommandBuilder, IntegerBuilder, StringBuilder, SubCommandBuilder, UserBuilder,
};

const COLOR: u32 = 0x2ecc71;

/// Montant maximal par cotisation (propre ou sale) : protège les totaux d'une saisie aberrante.
const MAX_AMOUNT: i64 = 100_000_000;

pub fn definition() -> Command {
    CommandBuilder::new("cotisation", "Ce que tu rapportes au groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("ajouter", "Déclare une cotisation (argent propre et/ou sale)")
                .option(StringBuilder::new("description", "D'où vient l'argent").required(true).max_length(300))
                .option(IntegerBuilder::new("propre", "Montant en argent propre").min_value(0).max_value(MAX_AMOUNT))
                .option(IntegerBuilder::new("sale", "Montant en argent sale").min_value(0).max_value(MAX_AMOUNT))
                .option(StringBuilder::new("date", "Date JJ/MM/AAAA (défaut : aujourd'hui)"))
                .option(UserBuilder::new("membre", "Déclarer pour un autre membre (gradés et leads)")),
        )
        .option(
            SubCommandBuilder::new("historique", "Tes dernières cotisations et ton total")
                .option(UserBuilder::new("membre", "Un autre membre (gradés et leads)"))
                .option(IntegerBuilder::new("jours", "Seulement les N derniers jours").min_value(1).max_value(3650)),
        )
        .option(
            SubCommandBuilder::new("stats", "Statistiques du groupe")
                .option(
                    IntegerBuilder::new("jours", "Les N derniers jours (défaut : depuis le début)")
                        .min_value(1)
                        .max_value(3650),
                )
                .option(UserBuilder::new("membre", "Un seul membre")),
        )
        .option(
            SubCommandBuilder::new("supprimer", "Supprime une cotisation saisie par erreur")
                .option(StringBuilder::new("cotisation", "Cotisation à supprimer").required(true).autocomplete(true)),
        )
        .option(SubCommandBuilder::new("objectif", "Où tu en es de l'objectif de la semaine"))
        .build()
}

/// `/config cotisation`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("cotisation", "Cotisations : annonces, objectif, tableau de bord (sans option : affiche)")
        .option(
            ChannelBuilder::new("salon", "Salon où annoncer chaque cotisation").channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("sans_annonce", "Ne plus annoncer les cotisations"))
        .option(
            IntegerBuilder::new("objectif", "Objectif hebdomadaire par membre, en $ (0 : pas d'objectif)")
                .min_value(0)
                .max_value(objective::MAX_OBJECTIVE),
        )
        .option(StringBuilder::new("compte", "Ce qui compte pour l'objectif (défaut : propre + sale)").choices([
            ("Propre + sale", Counts::Total.as_str()),
            ("Argent propre", Counts::Propre.as_str()),
            ("Argent sale", Counts::Sale.as_str()),
        ]))
        .option(BooleanBuilder::new("rappel", "Rappel le dimanche à 18:00 aux membres sous l'objectif (défaut : oui)"))
        .option(
            ChannelBuilder::new("tableau", "Salon du tableau permanent (classement et objectif)")
                .channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("sans_tableau", "Retirer le tableau de bord"))
}

/// Réglages actuels, pour `/config cotisation` sans option.
async fn config_summary(ix: &Ix, guild: &str) -> Result<String> {
    let mut lines = vec![match service::channel(&ix.ctx.db, guild).await? {
        Some(channel) => format!("Les cotisations sont annoncées dans <#{channel}>."),
        None => "Les cotisations ne sont annoncées dans aucun salon.".to_owned(),
    }];
    lines.push(match objective::get_objective(&ix.ctx.db, guild).await? {
        Some(o) => format!(
            "Objectif hebdomadaire : **{}**, rappel du dimanche {}.",
            o.describe(),
            if o.reminder { "activé" } else { "désactivé" }
        ),
        None => "Pas d'objectif hebdomadaire (`/config cotisation objectif:…`).".to_owned(),
    });
    lines.push(match dashboard::get_dashboard(&ix.ctx.db, guild).await?.0 {
        Some(channel) => format!("Tableau de bord publié dans <#{channel}>."),
        None => "Pas de tableau de bord (`/config cotisation tableau:#…`).".to_owned(),
    });
    Ok(lines.join("\n"))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    if options.summary().is_empty() {
        return ix.reply(Reply::ephemeral(config_summary(ix, guild).await?)).await;
    }
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    let mut lines = Vec::new();

    let channel = options.channel("salon").map(|id| id.to_string());
    if channel.is_some() || options.boolean("sans_annonce") == Some(true) {
        service::set_channel(&ix.ctx.db, guild, channel.as_deref()).await?;
        lines.push(match channel {
            Some(channel) => format!("Les cotisations seront annoncées dans <#{channel}>."),
            None => "Les cotisations ne seront plus annoncées.".to_owned(),
        });
    }

    let (amount, counts, reminder) = (
        options.integer("objectif"),
        options.string("compte").and_then(|c| Counts::parse(&c)),
        options.boolean("rappel"),
    );
    if amount.is_some() || counts.is_some() || reminder.is_some() {
        let current = objective::get_objective(&ix.ctx.db, guild).await?;
        match (amount, current) {
            (None, None) => lines.push("Indique d'abord le montant de l'objectif (`objectif:…`).".to_owned()),
            (amount, current) => {
                let updated = Objective {
                    amount: amount.or(current.map(|c| c.amount)).unwrap_or(0),
                    counts: counts.or(current.map(|c| c.counts)).unwrap_or_default(),
                    reminder: reminder.or(current.map(|c| c.reminder)).unwrap_or(true),
                };
                objective::set_objective(&ix.ctx.db, guild, &updated).await?;
                lines.push(if updated.amount > 0 {
                    format!(
                        "Objectif hebdomadaire : **{}** par membre, rappel du dimanche {}.",
                        updated.describe(),
                        if updated.reminder { "activé" } else { "désactivé" }
                    )
                } else {
                    "Plus d'objectif hebdomadaire.".to_owned()
                });
            }
        }
    }

    let board = options.channel("tableau").map(|id| id.to_string());
    let board_changed = board.is_some() || options.boolean("sans_tableau") == Some(true);
    if board_changed {
        // L'ancien message disparaît : changement de salon ou retrait du tableau.
        if let (Some(old_channel), Some(old_message)) = dashboard::get_dashboard(&ix.ctx.db, guild).await?
            && let (Some(c), Some(m)) = (parse_id(&old_channel), parse_id(&old_message))
        {
            messages::delete_quietly(&ix.ctx, c, m).await;
        }
        dashboard::set_dashboard_channel(&ix.ctx.db, guild, board.as_deref()).await?;
        lines.push(match &board {
            Some(channel) => format!("Tableau de bord publié dans <#{channel}>."),
            None => "Le tableau de bord n'est plus publié.".to_owned(),
        });
    }
    if lines.is_empty() {
        lines.push("Rien à changer.".to_owned());
    }
    ix.reply(Reply::ephemeral(lines.join("\n"))).await?;
    // Après la réponse : la publication peut prendre plus de 3 secondes.
    dashboard::refresh(&ix.ctx, guild_id).await;
    Ok(())
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    match options.sub() {
        "ajouter" => add(ix, options, guild_id).await,
        "historique" => history(ix, options, guild_id).await,
        "stats" => stats(ix, options, guild_id).await,
        "supprimer" => remove(ix, options, guild_id).await,
        "objectif" => objective_view(ix, guild_id).await,
        _ => Ok(()),
    }
}

/// `/cotisation objectif` : son avancement ; les gradés (`cotisation.stats`) voient tout le monde.
async fn objective_view(ix: &Ix, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let Some(objective) = objective::get_objective(&ix.ctx.db, &guild).await? else {
        return ix.reply(Reply::ephemeral("Aucun objectif hebdomadaire n'est fixé.")).await;
    };
    let week = week_start(Utc::now());
    let me = objective::member_progress(&ix.ctx.db, &guild, &ix.user_id().to_string(), week, &objective).await?;
    let mut description = if me.exempt {
        format!(
            "Tu n'es pas concerné cette semaine (absence déclarée ou recrue). Tes cotisations : **{}**.",
            money(me.amount)
        )
    } else if me.reached {
        format!("✅ Objectif atteint : **{}** / {}.", money(me.amount), money(me.target))
    } else {
        format!(
            "**{}** / {} — {}\nIl te reste **{}** avant dimanche minuit.",
            money(me.amount),
            money(me.target),
            objective::progress_bar(me.amount, me.target),
            money(me.target - me.amount)
        )
    };
    if permissions::can(ix, Perm::CotisationStats).await? {
        let rows = objective::objective_progress(&ix.ctx, guild_id, week).await?;
        let tracked: Vec<_> = rows.iter().filter(|r| !r.exempt).collect();
        let reached = tracked.iter().filter(|r| r.reached).count();
        let lines: Vec<String> = rows
            .iter()
            .map(|r| {
                let mark = if r.exempt {
                    "🌙"
                } else if r.reached {
                    "✅"
                } else {
                    "⏳"
                };
                format!("{mark} <@{}> — {} / {}", r.user_id, money(r.amount), money(r.target))
            })
            .collect();
        description.push_str(&format!("\n\n**Membres suivis : {reached}/{} ont atteint l'objectif**\n", tracked.len()));
        description.push_str(&if lines.is_empty() {
            "*Aucun membre suivi : règle le rôle membre avec `/config permission role_membre:@…`.*".to_owned()
        } else {
            lines.join("\n")
        });
    }
    let embed = EmbedBuilder::new()
        .title(format!("🎯 Objectif de la semaine {}", objective::week_label(week)))
        .color(COLOR)
        .description(truncate(&description, DESCRIPTION_LIMIT))
        .footer(footer(format!("Objectif : {} par membre", objective.describe())))
        .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

fn name(ix: &Ix, guild_id: Id<GuildMarker>, user_id: &str) -> String {
    parse_id(user_id).and_then(|id| display_name(&ix.ctx, guild_id, id)).unwrap_or_else(|| format!("<@{user_id}>"))
}

/// « 1 000 $ propre · 500 $ sale ».
fn amounts(propre: i64, sale: i64) -> String {
    match (propre, sale) {
        (0, sale) => format!("{} sale", money(sale)),
        (propre, 0) => format!("{} propre", money(propre)),
        (propre, sale) => format!("{} propre · {} sale", money(propre), money(sale)),
    }
}

async fn add(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let target = options.user("membre").unwrap_or(ix.user_id());
    let for_someone_else = target != ix.user_id();
    let perm = if for_someone_else { Perm::CotisationGerer } else { Perm::CotisationAjouter };
    if !permissions::require(ix, perm).await? {
        return Ok(());
    }
    if !permissions::require_member(ix, target, &options.user_name(target)).await? {
        return Ok(());
    }
    let propre = options.integer("propre").unwrap_or(0);
    let sale = options.integer("sale").unwrap_or(0);
    if !(0..=MAX_AMOUNT).contains(&propre) || !(0..=MAX_AMOUNT).contains(&sale) {
        return ix.reply(Reply::ephemeral("Montant invalide.")).await;
    }
    if propre + sale <= 0 {
        return ix.reply(Reply::ephemeral("Indique un montant en argent propre et/ou sale.")).await;
    }
    let date_label = options.string("date").unwrap_or_else(today_label);
    let Some(date) = parse_event_date(date_label.trim(), "00:00") else {
        return ix.reply(Reply::ephemeral("Date invalide. Format attendu : `JJ/MM/AAAA`.")).await;
    };
    if date.0 > DbTime::now().0 {
        return ix.reply(Reply::ephemeral("La date ne peut pas être dans le futur.")).await;
    }
    let description = options.string("description").unwrap_or_default().trim().to_owned();

    ix.defer(true).await?;
    let guild = guild_id.to_string();
    let cotisation = service::add(
        &ix.ctx.db,
        &guild,
        NewCotisation {
            user_id: target.to_string(),
            propre,
            sale,
            description: description.clone(),
            date,
            created_by_id: ix.user_id().to_string(),
        },
    )
    .await?;

    let mut embed = EmbedBuilder::new()
        .title("💰 Nouvelle cotisation")
        .color(COLOR)
        .field(field("Membre", format!("<@{target}>"), true))
        .field(field("Date", format_day_label(date), true))
        .field(field("Montant", amounts(propre, sale), false))
        .field(field("Description", truncate_field(&description), false))
        .maybe_thumbnail(options.resolved_user(target).or_else(|| ix.user().cloned()).map(|u| avatar_url(&u)));
    if for_someone_else {
        embed = embed.footer(footer(format!("Déclarée par {}", ix.username())));
    }
    let embed = embed.build();

    if let Some(channel_id) = service::channel(&ix.ctx.db, &guild).await?.as_deref().and_then(parse_id)
        && let Err(error) = messages::send(&ix.ctx, channel_id, &Outgoing::embeds(vec![embed.clone()])).await
    {
        tracing::warn!(?error, "annonce de cotisation impossible");
    }
    if for_someone_else {
        audit::record(
            ix,
            Entry::new(
                Action::CotisationAddFor,
                Some(target.to_string()),
                format!("{} — {description}", amounts(propre, sale)),
            ),
        )
        .await;
    }

    let total =
        service::totals(&ix.ctx.db, &guild, &Filter { user_id: Some(target.to_string()), ..Default::default() })
            .await?;
    ix.edit(
        Reply::text(format!(
            "Cotisation enregistrée (`{}`). Total de <@{target}> : **{}**.",
            cotisation.id,
            money(total.total())
        ))
        .with_embed(embed),
    )
    .await?;
    dashboard::refresh(&ix.ctx, guild_id).await;
    Ok(())
}

fn since_days(days: Option<i64>) -> Option<DbTime> {
    days.map(|days| DbTime(DbTime::now().0 - Duration::days(days)))
}

fn period_label(days: Option<i64>) -> String {
    match days {
        Some(1) => "sur les dernières 24 h".to_owned(),
        Some(days) => format!("sur les {days} derniers jours"),
        None => "depuis le début".to_owned(),
    }
}

fn totals_fields(embed: EmbedBuilder, totals: &Totals) -> EmbedBuilder {
    embed
        .field(field("🧼 Propre", money(totals.propre), true))
        .field(field("💸 Sale", money(totals.sale), true))
        .field(field("Total", format!("**{}**", money(totals.total())), true))
}

async fn history(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let target = options.user("membre").unwrap_or(ix.user_id());
    if target != ix.user_id() && !permissions::require(ix, Perm::CotisationGerer).await? {
        return Ok(());
    }
    let days = options.integer("jours");
    let guild = guild_id.to_string();
    let filter = Filter { since: since_days(days), user_id: Some(target.to_string()), ..Default::default() };
    let totals = service::totals(&ix.ctx.db, &guild, &filter).await?;
    let rows = service::list(&ix.ctx.db, &guild, &filter, 15, 0).await?;

    let description = if rows.is_empty() {
        "Aucune cotisation.".to_owned()
    } else {
        truncate(
            &rows
                .iter()
                .map(|c| format!("• {} — {} — {}", format_day_label(c.date), amounts(c.propre, c.sale), c.description))
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let embed = totals_fields(
        EmbedBuilder::new()
            .title(format!("💰 Cotisations de {}", name(ix, guild_id, &target.to_string())))
            .color(COLOR)
            .description(description),
        &totals,
    )
    .footer(footer(format!("{} cotisation(s) {}", totals.count, period_label(days))))
    .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn stats(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    if !permissions::require(ix, Perm::CotisationStats).await? {
        return Ok(());
    }
    let days = options.integer("jours");
    let guild = guild_id.to_string();
    let filter = Filter {
        since: since_days(days),
        user_id: options.user("membre").map(|id| id.to_string()),
        ..Default::default()
    };
    let totals = service::totals(&ix.ctx.db, &guild, &filter).await?;
    let members = service::by_member(&ix.ctx.db, &guild, &filter).await?;

    let ranking = if members.is_empty() {
        "Aucune cotisation sur la période.".to_owned()
    } else {
        truncate(
            &members
                .iter()
                .enumerate()
                .map(|(rank, (user, t))| {
                    format!(
                        "{}. <@{user}> — **{}** ({} propre · {} sale · {} cotis.)",
                        rank + 1,
                        money(t.total()),
                        money(t.propre),
                        money(t.sale),
                        t.count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            DESCRIPTION_LIMIT,
        )
    };
    let mut footer_text = format!("{} cotisation(s) {}", totals.count, period_label(days));
    if let Some(url) = crate::web::auth::public_url(&ix.ctx) {
        footer_text.push_str(&format!(" · détail : {url}/stats/{guild}/cotisations"));
    }
    let embed =
        totals_fields(EmbedBuilder::new().title("📊 Cotisations du groupe").color(COLOR).description(ranking), &totals)
            .footer(footer(footer_text))
            .build();
    ix.reply(Reply::embed(embed).hidden()).await
}

async fn remove(ix: &Ix, options: &Options, guild_id: Id<GuildMarker>) -> Result<()> {
    let guild = guild_id.to_string();
    let id = options.string("cotisation").unwrap_or_default();
    let Some(cotisation) = service::get(&ix.ctx.db, &guild, &id).await? else {
        return ix.reply(Reply::ephemeral("Cotisation introuvable.")).await;
    };
    // Une cotisation déclarée par un gradé pour un membre ne peut être retirée que par le staff.
    let own = cotisation.created_by_id == ix.user_id().to_string();
    let officer = permissions::can(ix, Perm::CotisationGerer).await?;
    if !own && !officer {
        return ix
            .reply(Reply::ephemeral("Tu ne peux supprimer que les cotisations que tu as déclarées toi-même."))
            .await;
    }
    service::delete(&ix.ctx.db, &guild, &cotisation.id).await?;
    audit::record(
        ix,
        Entry::new(
            Action::CotisationDelete,
            Some(cotisation.user_id.clone()),
            format!(
                "{} — {} — {}",
                format_day_label(cotisation.date),
                amounts(cotisation.propre, cotisation.sale),
                cotisation.description
            ),
        ),
    )
    .await;
    ix.reply(Reply::ephemeral(format!(
        "Cotisation supprimée : {} du {}.",
        amounts(cotisation.propre, cotisation.sale),
        format_day_label(cotisation.date)
    )))
    .await?;
    dashboard::refresh(&ix.ctx, guild_id).await;
    Ok(())
}

pub async fn autocomplete(ix: &Ix, options: &Options) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.autocomplete(Vec::new()).await;
    };
    // Les gradés voient toutes les cotisations récentes, les membres les leurs.
    let user_id =
        if permissions::can(ix, Perm::CotisationGerer).await? { None } else { Some(ix.user_id().to_string()) };
    let query = options.focused.as_ref().map(|(_, v)| v.to_lowercase()).unwrap_or_default();
    let rows =
        service::list(&ix.ctx.db, &guild_id.to_string(), &Filter { user_id, ..Default::default() }, 100, 0).await?;
    let choices = rows
        .iter()
        .filter(|c| query.is_empty() || c.description.to_lowercase().contains(&query))
        .take(25)
        .map(|c| {
            let who = name(ix, guild_id, &c.user_id);
            choice(
                &format!("{} · {} · {} · {}", format_day_label(c.date), who, amounts(c.propre, c.sale), c.description),
                c.id.clone(),
            )
        })
        .collect();
    ix.autocomplete(choices).await
}
