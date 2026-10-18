use super::{
    money,
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
        time::{DbTime, format_day_label, parse_event_date, today_label},
    },
};
use anyhow::Result;
use chrono::Duration;
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

pub fn definition() -> Command {
    CommandBuilder::new("cotisation", "Ce que tu rapportes au groupe", CommandType::ChatInput)
        .option(
            SubCommandBuilder::new("ajouter", "Déclare une cotisation (argent propre et/ou sale)")
                .option(StringBuilder::new("description", "D'où vient l'argent").required(true).max_length(300))
                .option(IntegerBuilder::new("propre", "Montant en argent propre").min_value(0))
                .option(IntegerBuilder::new("sale", "Montant en argent sale").min_value(0))
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
        .build()
}

/// `/config cotisation`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("cotisation", "Cotisations : salon des annonces (sans option : affiche)")
        .option(
            ChannelBuilder::new("salon", "Salon où annoncer chaque cotisation").channel_types([ChannelType::GuildText]),
        )
        .option(BooleanBuilder::new("sans_annonce", "Ne plus annoncer les cotisations"))
}

pub async fn config(ix: &Ix, options: &Options, guild: &str) -> Result<()> {
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let channel = options.channel("salon").map(|id| id.to_string());
    if channel.is_none() && options.boolean("sans_annonce") != Some(true) {
        let current = service::channel(&ix.ctx.db, guild).await?;
        return ix
            .reply(Reply::ephemeral(match current {
                Some(channel) => format!("Les cotisations sont annoncées dans <#{channel}>."),
                None => "Les cotisations ne sont annoncées dans aucun salon.".to_owned(),
            }))
            .await;
    }
    service::set_channel(&ix.ctx.db, guild, channel.as_deref()).await?;
    ix.reply(Reply::ephemeral(match channel {
        Some(channel) => format!("Les cotisations seront annoncées dans <#{channel}>."),
        None => "Les cotisations ne seront plus annoncées.".to_owned(),
    }))
    .await
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
        _ => Ok(()),
    }
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
    let propre = options.integer("propre").unwrap_or(0);
    let sale = options.integer("sale").unwrap_or(0);
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
    .await
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
    let own = cotisation.created_by_id == ix.user_id().to_string() || cotisation.user_id == ix.user_id().to_string();
    let officer = permissions::can(ix, Perm::CotisationGerer).await?;
    if !own && !officer {
        return ix.reply(Reply::ephemeral("Tu ne peux supprimer que tes propres cotisations.")).await;
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
    .await
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
