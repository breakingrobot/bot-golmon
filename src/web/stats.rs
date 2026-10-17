//! Statistiques d'un serveur : assiduité aux présences, recrues de la semaine,
//! activité des modules. Réservé à « Gérer le serveur ».

use super::{
    MaybeViewer,
    pages::{self, Layout},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::{presences, recrues},
};
use anyhow::Result;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
};
use twilight_model::id::{Id, marker::GuildMarker};

struct Figure {
    label: &'static str,
    value: String,
    hint: Option<String>,
}

struct Attendance {
    name: String,
    present: i64,
    absent: i64,
    retard: i64,
    percent: i64,
}

struct Recruit {
    name: String,
    pour: usize,
    contre: usize,
    blanc: usize,
    veto: Option<String>,
}

#[derive(Template)]
#[template(path = "stats.html")]
struct StatsPage {
    layout: Layout,
    guild_name: String,
    figures: Vec<Figure>,
    attendance: Vec<Attendance>,
    recruits: Vec<Recruit>,
    tickets: Option<crate::modmail::TicketStats>,
}

pub async fn index(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri) -> Response {
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, uri.path()).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    match access.guilds.as_slice() {
        [] => pages::forbidden(&ctx, &viewer, &access),
        [(id, _)] => Redirect::to(&format!("/stats/{id}")).into_response(),
        _ => Redirect::to("/").into_response(),
    }
}

pub async fn guild(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri, Path(guild_id): Path<u64>) -> Response {
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, uri.path()).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Some(guild_id) = Id::<GuildMarker>::new_checked(guild_id) else {
        return pages::not_found().await;
    };
    if !access.can_view_guild(guild_id) {
        return pages::forbidden(&ctx, &viewer, &access);
    }
    let guild_name = access.guilds.iter().find(|(id, _)| *id == guild_id).map(|(_, n)| n.clone()).unwrap_or_default();
    match build(&ctx, guild_id).await {
        Ok((figures, attendance, recruits, tickets)) => pages::render(&StatsPage {
            layout: Layout::new(&ctx, format!("Statistiques · {guild_name}"), Some(&viewer), &access, "/stats"),
            guild_name,
            figures,
            attendance,
            recruits,
            tickets,
        }),
        Err(error) => {
            tracing::error!(?error, %guild_id, "statistiques");
            pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Les statistiques sont indisponibles.")
        }
    }
}

async fn count(ctx: &Ctx, sql: &str, guild_id: &str) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(sql).bind(guild_id).fetch_one(&ctx.db).await?;
    Ok(n)
}

type Built = (Vec<Figure>, Vec<Attendance>, Vec<Recruit>, Option<crate::modmail::TicketStats>);

async fn build(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<Built> {
    let gid = guild_id.to_string();
    let name = |user_id: &str| {
        user_id
            .parse()
            .ok()
            .and_then(|id| members::display_name(ctx, guild_id, id))
            .unwrap_or_else(|| user_id.to_owned())
    };

    let presences_total =
        count(ctx, r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL"#, &gid).await?;
    let presences_month = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL
           AND "eventDate" >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-30 days')"#,
        &gid,
    )
    .await?;
    let absences = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Absence" WHERE "guildId" = ? AND "endedAt" IS NULL
           AND "startsAt" <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
        &gid,
    )
    .await?;
    let transactions = count(
        ctx,
        r#"SELECT COUNT(*) FROM "Transaction" WHERE "guildId" = ? AND "statut" NOT IN ('LIVRE_PAYE', 'ANNULEE', 'CLOTUREE')"#,
        &gid,
    )
    .await?;
    let vehicles = count(ctx, r#"SELECT COUNT(*) FROM "Vehicle" WHERE "guildId" = ?"#, &gid).await?;

    let leaderboard = presences::service::leaderboard(&ctx.db, &gid).await?;
    let average = if leaderboard.is_empty() {
        0
    } else {
        leaderboard.iter().map(|e| e.percent).sum::<i64>() / leaderboard.len() as i64
    };
    let attendance: Vec<Attendance> = leaderboard
        .into_iter()
        .map(|e| Attendance {
            name: name(&e.user_id),
            present: e.present,
            absent: e.absent,
            retard: e.retard,
            percent: e.percent,
        })
        .collect();

    let session = recrues::service::current_session(&ctx.db, &gid).await?;
    let recruits: Vec<Recruit> = session
        .map(|s| s.entries)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            let votes = |choix: &str| entry.votes.iter().filter(|v| v.choix == choix).count();
            Recruit {
                name: name(&entry.user_id),
                pour: votes("POUR"),
                contre: votes("CONTRE"),
                blanc: votes("BLANC"),
                veto: entry
                    .veto_by_id
                    .as_ref()
                    .map(|lead| format!("{} : {}", name(lead), entry.veto_motif.clone().unwrap_or_default())),
            }
        })
        .collect();

    let figures = vec![
        Figure {
            label: "Présences clôturées",
            value: presences_total.to_string(),
            hint: Some(format!("{presences_month} ces 30 derniers jours")),
        },
        Figure { label: "Taux de présence moyen", value: format!("{average} %"), hint: None },
        Figure { label: "Absences en cours", value: absences.to_string(), hint: None },
        Figure { label: "Recrues en essai", value: recruits.len().to_string(), hint: None },
        Figure { label: "Transactions en cours", value: transactions.to_string(), hint: None },
        Figure { label: "Véhicules enregistrés", value: vehicles.to_string(), hint: None },
    ];

    let tickets = match &ctx.modmail {
        Some(modmail) if modmail.config.main_server_ids.contains(&guild_id) => Some(modmail.ticket_stats(ctx).await?),
        _ => None,
    };

    Ok((figures, attendance, recruits, tickets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modmail::{StaffActivity, TicketStats};

    #[test]
    fn renders_stats_page() {
        let page = StatsPage {
            layout: Layout::bare("Statistiques"),
            guild_name: "Serveur RP".into(),
            figures: vec![
                Figure {
                    label: "Présences clôturées",
                    value: "42".into(),
                    hint: Some("8 ces 30 derniers jours".into()),
                },
                Figure { label: "Taux de présence moyen", value: "71 %".into(), hint: None },
                Figure { label: "Recrues en essai", value: "2".into(), hint: None },
            ],
            attendance: vec![
                Attendance { name: "Elodie".into(), present: 38, absent: 2, retard: 2, percent: 90 },
                Attendance { name: "Marco".into(), present: 25, absent: 12, retard: 5, percent: 60 },
                Attendance { name: "Sam".into(), present: 10, absent: 30, retard: 2, percent: 24 },
            ],
            recruits: vec![
                Recruit { name: "Lina".into(), pour: 4, contre: 1, blanc: 0, veto: None },
                Recruit {
                    name: "Hugo".into(),
                    pour: 1,
                    contre: 3,
                    blanc: 1,
                    veto: Some("Marco : absences répétées".into()),
                },
            ],
            tickets: Some(TicketStats {
                open: 3,
                opened: 27,
                closed: 24,
                unanswered: 1,
                median_first_response: Some("14 min".into()),
                staff: vec![StaffActivity {
                    name: "Marco".into(),
                    replies: 58,
                    tickets: 19,
                    claims: 12,
                    first_responses: 15,
                    average_first_response: Some("11 min".into()),
                }],
            }),
        };
        let html = page.render().unwrap();
        assert!(html.contains("Assiduité aux présences") && html.contains("Véto · Marco"));
        assert!(html.contains("14 min") && html.contains("Pris en charge"));
        if let Ok(path) = std::env::var("STATS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, crate::web::ASSET_VERSION),
                &format!("<style>{}</style>", crate::web::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
