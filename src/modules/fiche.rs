//! « Fiche du membre » (clic droit sur un membre, ou `/fiche [membre]`) : résumé
//! de tous les modules en un message privé — ancienneté, présences (globales et
//! 4 dernières semaines), absences, cotisations et objectif, transactions,
//! coffres, recrue, numéro, véhicules — avec le bouton vers sa fiche sur le site.
//! Réservée à `membre.fiche` (gradés par défaut) ; chacun peut voir la sienne.

use crate::{
    context::Ctx,
    discord::{
        embed::{EmbedBuilder, EmbedBuilderExt as _, footer},
        interaction::{GUILD_ONLY, Ix, Reply},
        members::fetch_display_name,
        options::Options,
        site::{self, Page},
    },
    modules::{
        coffres,
        cotisations::{self, money, objective, service::Filter},
        permissions::{self, Perm},
        phones,
        presences::{
            self,
            report::{Rate, TrackedMember},
        },
        recrues::{self, Vote},
        vehicles,
    },
    util::{
        format::{field, plural, truncate_field},
        time::{DbTime, format_day_label, week_start},
    },
};
use anyhow::Result;
use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::InteractionContextType,
    },
    id::{
        Id,
        marker::{GuildMarker, UserMarker},
    },
};
use twilight_util::builder::command::{CommandBuilder, UserBuilder};

const COLOR: u32 = 0x5865f2;
/// Fenêtres de la fiche : assiduité récente, absences, transactions et coffres.
const RECENT_WEEKS: i64 = presences::report::ALERT_WEEKS;
const ABSENCE_DAYS: i64 = 90;
const ACTIVITY_DAYS: i64 = 30;

/// `/fiche [membre]`.
pub fn definition() -> Command {
    CommandBuilder::new(
        "fiche",
        "Fiche d'un membre : présences, cotisations, coffres… (la tienne par défaut)",
        CommandType::ChatInput,
    )
    .contexts([InteractionContextType::Guild])
    .option(UserBuilder::new("membre", "Membre (gradés et leads ; toi par défaut)"))
    .build()
}

pub async fn handle(ix: &Ix, options: &Options) -> Result<()> {
    show(ix, options, options.user("membre").unwrap_or(ix.user_id())).await
}

/// « Sur le serveur depuis le 12/03/2026 (il y a 7 mois) », en horodatages Discord.
pub fn seniority_text(joined_at: Option<DbTime>) -> String {
    match joined_at {
        Some(joined) => format!("Depuis le <t:{0}:D> (<t:{0}:R>)", joined.unix()),
        None => "Date d'arrivée inconnue.".to_owned(),
    }
}

/// Champ des présences : taux global, détail, puis les 4 dernières semaines.
pub fn presence_text(stats: &presences::service::UserStats, recent: Rate) -> String {
    let mut text = if stats.total_closed == 0 {
        "Aucune présence clôturée.".to_owned()
    } else {
        format!(
            "**{} %** de présence · ✅ {} · ⏳ {} · ❌ {} · sans réponse {} (sur {})",
            stats.percent, stats.present, stats.retard, stats.absent, stats.sans_reponse, stats.total_closed
        )
    };
    text.push_str(&match recent.rounded() {
        Some(percent) => format!(
            "\n{RECENT_WEEKS} dernières semaines : **{percent} %** ({} présent{} sur {})",
            recent.present,
            plural(recent.present as usize),
            recent.expected
        ),
        None => format!("\n{RECENT_WEEKS} dernières semaines : aucune présence attendue."),
    });
    text
}

/// Champ des absences : en cours ou à venir, et nombre déclaré sur 90 jours.
pub fn absence_text(current: Option<String>, declared: i64) -> String {
    let mut text = current.unwrap_or_else(|| "Aucune en cours.".to_owned());
    text.push_str(&format!("\n{declared} déclarée{} sur {ABSENCE_DAYS} jours", plural(declared as usize)));
    text
}

/// Absences déclarées dont la période chevauche les 90 derniers jours (une
/// absence annulée avant son début ne compte pas).
async fn absences_declared(db: &SqlitePool, guild: &str, user: &str) -> Result<i64> {
    let now = DbTime::now();
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "Absence"
           WHERE "guildId" = ? AND "userId" = ? AND "startsAt" < ?
             AND MIN("endsAt", COALESCE("endedAt", "endsAt")) > ? AND MIN("endsAt", COALESCE("endedAt", "endsAt")) > "startsAt""#,
    )
    .bind(guild)
    .bind(user)
    .bind(now)
    .bind(DbTime(now.0 - Duration::days(ABSENCE_DAYS)))
    .fetch_one(db)
    .await?;
    Ok(count)
}

/// Transactions créées par le membre sur les 30 derniers jours.
async fn transactions_created(db: &SqlitePool, guild: &str, user: &str) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "Transaction" WHERE "guildId" = ? AND "createdById" = ? AND "createdAt" >= ?"#,
    )
    .bind(guild)
    .bind(user)
    .bind(DbTime(Utc::now() - Duration::days(ACTIVITY_DAYS)))
    .fetch_one(db)
    .await?;
    Ok(count)
}

/// Dernier dépôt ou retrait d'un des personnages reliés.
async fn last_coffre_activity(db: &SqlitePool, guild: &str, game_ids: &[String]) -> Result<Option<DbTime>> {
    if game_ids.is_empty() {
        return Ok(None);
    }
    let sql = format!(
        r#"SELECT MAX("at") FROM "CoffreMovement" WHERE "guildId" = ? AND "kind" IN ('DEPOSIT', 'WITHDRAW') AND "playerGameId" IN ({})"#,
        vec!["?"; game_ids.len()].join(", ")
    );
    let mut query = sqlx::query_as::<_, (Option<DbTime>,)>(&sql).bind(guild);
    for id in game_ids {
        query = query.bind(id);
    }
    Ok(query.fetch_one(db).await?.0)
}

/// Arrivée sur le serveur et rôles du membre : cache, sinon Discord.
async fn member_info(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> (Option<DbTime>, Vec<String>) {
    let to_time = |joined: twilight_model::util::Timestamp| {
        chrono::DateTime::from_timestamp_micros(joined.as_micros()).map(DbTime)
    };
    if let Some(member) = ctx.cache.member(guild_id, user_id) {
        return (member.joined_at().and_then(to_time), member.roles().iter().map(ToString::to_string).collect());
    }
    match ctx.http.guild_member(guild_id, user_id).await {
        Ok(response) => match response.model().await {
            Ok(member) => (member.joined_at.and_then(to_time), member.roles.iter().map(ToString::to_string).collect()),
            Err(_) => (None, Vec::new()),
        },
        Err(_) => (None, Vec::new()),
    }
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

    let (joined_at, roles) = member_info(&ix.ctx, guild_id, target).await;
    let mut embed = EmbedBuilder::new()
        .title(format!("🪪 Fiche de {name}"))
        .color(COLOR)
        .description(format!("<@{target}> · {}", seniority_text(joined_at)))
        .maybe_thumbnail(options.resolved_user(target).map(|u| crate::discord::members::avatar_url(&u)));

    // Présences : global et 4 dernières semaines (règle du rapport hebdomadaire).
    let stats = presences::service::user_stats(db, &guild, &user).await?;
    let presence_role = presences::service::get_config(db, &guild).await?.and_then(|c| c.default_role_id);
    let window =
        presences::report::gather(db, &guild, DbTime(Utc::now() - Duration::weeks(RECENT_WEEKS)), DbTime::now())
            .await?;
    let tracked = TrackedMember { user_id: user.clone(), joined_at };
    let holds_role = presence_role.is_some_and(|role| roles.contains(&role));
    let recent = presences::report::member_rate(&window, &tracked, holds_role);
    embed = embed.field(field("📋 Présences", presence_text(&stats, recent), false));

    // Absence en cours ou à venir, et absences déclarées récemment.
    let absence = match presences::absences::active_absence(db, &guild, &user).await? {
        Some(a) => Some(format!("🌙 En cours jusqu'au {} — {}", format_day_label(a.ends_at), a.motif)),
        None => presences::absences::upcoming_absence(db, &guild, &user)
            .await?
            .map(|a| format!("🗓️ Prévue, jusqu'au {} — {}", format_day_label(a.ends_at), a.motif)),
    };
    let declared = absences_declared(db, &guild, &user).await?;
    embed = embed.field(field("🌙 Absences", truncate_field(&absence_text(absence, declared)), true));

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

    // Transactions créées récemment.
    let created = transactions_created(db, &guild, &user).await?;
    embed = embed.field(field(
        "🤝 Transactions",
        format!("{created} créée{} sur {ACTIVITY_DAYS} jours", plural(created as usize)),
        true,
    ));

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
        let last = match last_coffre_activity(db, &guild, &game_ids).await? {
            Some(at) => format!("\nDernier mouvement <t:{}:R>", at.unix()),
            None => "\nAucun mouvement enregistré.".to_owned(),
        };
        embed = embed.field(field(
            "📦 Coffres (30 jours)",
            truncate_field(&format!(
                "{}\n📥 {deposited} déposé(s) · 📤 {withdrawn} retiré(s) · {count} mouvement(s){last}",
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

    let embed = embed.footer(footer("Visible par toi seul")).build();
    let link = site::row_for(&ix.ctx, guild_id, Page::Membre(target));
    ix.edit(Reply::embed(embed).with_components(link)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::presences::service::UserStats;

    fn stats(total: i64, present: i64) -> UserStats {
        UserStats {
            total_closed: total,
            present,
            absent: 1,
            retard: 0,
            sans_reponse: total - present - 1,
            percent: present * 100 / total.max(1),
            recent_changes: Vec::new(),
        }
    }

    #[test]
    fn formats_presence_summary() {
        let text = presence_text(&stats(10, 8), Rate { present: 3, expected: 4 });
        assert_eq!(
            text,
            "**80 %** de présence · ✅ 8 · ⏳ 0 · ❌ 1 · sans réponse 1 (sur 10)\n4 dernières semaines : **75 %** (3 présents sur 4)"
        );
        let empty = presence_text(&stats(0, 0), Rate::default());
        assert_eq!(empty, "Aucune présence clôturée.\n4 dernières semaines : aucune présence attendue.");
    }

    #[test]
    fn formats_absences_and_seniority() {
        assert_eq!(absence_text(None, 0), "Aucune en cours.\n0 déclarée sur 90 jours");
        assert_eq!(absence_text(Some("🌙 En cours".into()), 2), "🌙 En cours\n2 déclarées sur 90 jours");
        assert_eq!(
            seniority_text(Some(DbTime(chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap()))),
            "Depuis le <t:1700000000:D> (<t:1700000000:R>)"
        );
        assert_eq!(seniority_text(None), "Date d'arrivée inconnue.");
    }

    #[test]
    fn slash_command_is_valid() {
        twilight_validate::command::command(&definition()).unwrap();
    }
}
