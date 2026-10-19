//! « Fiche du membre » (clic droit sur un membre) : résumé de tous les modules
//! en un message privé — présences, absence, cotisations et objectif, coffres,
//! recrue, numéro, véhicules — avec le lien vers sa fiche sur le site.
//! Réservée à `membre.fiche` (gradés par défaut) ; chacun peut voir la sienne.

use crate::{
    discord::{
        embed::{EmbedBuilder, EmbedBuilderExt as _, footer},
        interaction::{GUILD_ONLY, Ix, Reply},
        members::fetch_display_name,
        options::Options,
    },
    modules::{
        coffres,
        cotisations::{self, money, objective, service::Filter},
        permissions::{self, Perm},
        phones, presences,
        recrues::{self, Vote},
        vehicles,
    },
    util::{
        format::{field, truncate_field},
        time::{DbTime, format_day_label, week_start},
    },
};
use anyhow::Result;
use chrono::{Duration, Utc};
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};

const COLOR: u32 = 0x5865f2;

/// Lien vers la fiche du membre sur le site, si le site est configuré.
pub fn web_link(ctx: &crate::context::Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Option<String> {
    crate::web::auth::public_url(ctx).map(|url| format!("{url}/membres/{user_id}?serveur={guild_id}"))
}

pub async fn show(ix: &Ix, options: &Options, target: Id<UserMarker>) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if target != ix.user_id() && !permissions::require(ix, Perm::MembreFiche).await? {
        return Ok(());
    }
    ix.defer(true).await?;
    let db = &ix.ctx.db;
    let guild = guild_id.to_string();
    let user = target.to_string();
    let name = fetch_display_name(&ix.ctx, guild_id, target).await.unwrap_or_else(|| options.user_name(target));

    let mut embed = EmbedBuilder::new()
        .title(format!("🪪 Fiche de {name}"))
        .color(COLOR)
        .description(format!("<@{target}>"))
        .maybe_thumbnail(options.resolved_user(target).map(|u| crate::discord::members::avatar_url(&u)));

    // Présences.
    let stats = presences::service::user_stats(db, &guild, &user).await?;
    embed = embed.field(field(
        "📋 Présences",
        if stats.total_closed == 0 {
            "Aucune présence clôturée.".to_owned()
        } else {
            format!(
                "**{} %** de présence · ✅ {} · ⏳ {} · ❌ {} · sans réponse {} (sur {})",
                stats.percent, stats.present, stats.retard, stats.absent, stats.sans_reponse, stats.total_closed
            )
        },
        false,
    ));

    // Absence en cours ou à venir.
    let absence = match presences::absences::active_absence(db, &guild, &user).await? {
        Some(a) => Some(format!("🌙 En cours jusqu'au {} — {}", format_day_label(a.ends_at), a.motif)),
        None => presences::absences::upcoming_absence(db, &guild, &user)
            .await?
            .map(|a| format!("🗓️ Prévue, jusqu'au {} — {}", format_day_label(a.ends_at), a.motif)),
    };
    embed = embed.field(field("🌙 Absence", absence.unwrap_or_else(|| "Aucune.".to_owned()), true));

    // Cotisations : semaine, 30 jours, objectif.
    let week = week_start(Utc::now());
    let week_filter = Filter { since: Some(week), until: Some(objective::week_end(week)), user_id: Some(user.clone()) };
    let month_filter = Filter {
        since: Some(DbTime(Utc::now() - Duration::days(30))),
        user_id: Some(user.clone()),
        ..Default::default()
    };
    let this_week = cotisations::service::totals(db, &guild, &week_filter).await?;
    let month = cotisations::service::totals(db, &guild, &month_filter).await?;
    let mut text = format!(
        "Semaine : **{}** ({} propre · {} sale)\n30 jours : **{}** ({} cotis.)",
        money(this_week.total()),
        money(this_week.propre),
        money(this_week.sale),
        money(month.total()),
        month.count
    );
    if let Some(goal) = objective::get_objective(db, &guild).await? {
        let progress = objective::member_progress(db, &guild, &user, week, &goal).await?;
        text.push_str(&if progress.exempt {
            "\n🎯 Objectif : non concerné cette semaine".to_owned()
        } else if progress.reached {
            format!("\n🎯 Objectif atteint ({} / {})", money(progress.amount), money(progress.target))
        } else {
            format!("\n🎯 {}", objective::progress_bar(progress.amount, progress.target))
        });
    }
    embed = embed.field(field("💰 Cotisations", text, false));

    // Coffres : personnages reliés, activité sur 30 jours.
    let game_ids = coffres::service::game_ids_of(db, &guild, &user).await?;
    if !game_ids.is_empty() {
        let filter = coffres::service::Filter {
            player_game_ids: Some(game_ids.clone()),
            since: Some(DbTime(Utc::now() - Duration::days(30))),
            ..Default::default()
        };
        let players = coffres::service::by_player(db, &guild, &filter).await?;
        let (deposited, withdrawn, count) =
            players.iter().fold((0, 0, 0), |(d, w, c), (_, _, dep, wit, n)| (d + dep, w + wit, c + n));
        let names: Vec<String> = game_ids
            .iter()
            .map(|id| match players.iter().find(|(gid, ..)| gid == id) {
                Some((_, name, ..)) => format!("{name} ({id})"),
                None => format!("`{id}`"),
            })
            .collect();
        embed = embed.field(field(
            "📦 Coffres (30 jours)",
            truncate_field(&format!(
                "{}\n📥 {deposited} déposé(s) · 📤 {withdrawn} retiré(s) · {count} mouvement(s)",
                names.join(", ")
            )),
            false,
        ));
    }

    // Recrue : semaine en cours ou passé de recrue ; le détail des avis reste réservé au suivi.
    let history = recrues::service::user_history(db, &guild, &user).await?;
    if let Some((entry, entry_week)) = history.first() {
        let current = *entry_week == week;
        let mut text = if current {
            "🧑‍🎓 Recrue cette semaine".to_owned()
        } else {
            format!("Ancienne recrue (dernière semaine : {})", format_day_label(*entry_week))
        };
        if permissions::can(ix, Perm::RecrueSuivi).await? {
            let tally: Vec<String> = Vote::ORDER
                .iter()
                .map(|vote| {
                    let n = entry.votes.iter().filter(|v| v.choix == vote.as_str()).count();
                    format!("{} {n}", vote.emoji())
                })
                .collect();
            text.push_str(&format!(" · {}", tally.join(" ")));
            if entry.veto_by_id.is_some() {
                text.push_str(" · ⛔ véto");
            }
            if history.len() > 1 {
                text.push_str(&format!(" · {} semaines suivies", history.len()));
            }
        }
        embed = embed.field(field("🧑‍🎓 Recrue", text, false));
    }

    // Numéro et véhicules.
    let phone = phones::service::get(db, &guild, &user).await?.map(|p| phones::format(&p.numero));
    embed = embed.field(field("📞 Numéro", phone.unwrap_or_else(|| "—".to_owned()), true));
    let vehicles: Vec<String> = vehicles::service::list(db, &guild)
        .await?
        .into_iter()
        .filter(|v| !v.owner_is_role && v.owner_id == user)
        .map(|v| match v.modele {
            Some(model) => format!("`{}` {model}", v.plate),
            None => format!("`{}`", v.plate),
        })
        .collect();
    embed = embed.field(field(
        "🚗 Véhicules",
        if vehicles.is_empty() { "—".to_owned() } else { truncate_field(&vehicles.join("\n")) },
        true,
    ));

    if let Some(link) = web_link(&ix.ctx, guild_id, target) {
        embed = embed.field(field("🌐 Sur le site", format!("[Fiche complète]({link})"), false));
    }
    let embed = embed.footer(footer("Visible par toi seul")).build();
    ix.edit(Reply::embed(embed)).await
}
