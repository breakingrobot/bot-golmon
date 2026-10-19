//! Statistiques d'un serveur : assiduité aux présences, recrues de la semaine,
//! activité des modules. Accès : `site.stats`.

use super::{
    MaybeViewer,
    pages::{self, Layout},
};
use crate::{
    context::{Context, Ctx},
    discord::members,
    modules::permissions::Perm,
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
    /// Autres pages du serveur ouvertes à la personne (libellé, adresse).
    links: Vec<(&'static str, String)>,
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
    match access.guilds_with(Perm::SiteStats).as_slice() {
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
    if !access.can(guild_id, Perm::SiteStats) {
        return pages::forbidden(&ctx, &viewer, &access);
    }
    let guild_name = access.guild_name(guild_id);
    match build(&ctx, guild_id).await {
        Ok(data) => {
            let (figures, attendance, recruits, tickets) = for_page(&ctx, guild_id, data);
            pages::render(&StatsPage {
                layout: Layout::new(&ctx, format!("Statistiques · {guild_name}"), Some(&viewer), &access, "/stats"),
                guild_name,
                links: access.pages(guild_id).into_iter().filter(|(label, _)| *label != "Statistiques").collect(),
                figures,
                attendance,
                recruits,
                tickets,
            })
        }
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

/// Chiffre clé du serveur (page et API).
pub(crate) struct FigureData {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) hint: Option<String>,
}

/// Statistiques d'un serveur, membres en identifiants (page et API).
pub(crate) struct StatsData {
    pub(crate) figures: Vec<FigureData>,
    pub(crate) attendance: Vec<presences::service::LeaderboardEntry>,
    pub(crate) recruits: Vec<recrues::service::Entry>,
    pub(crate) tickets: Option<crate::modmail::TicketStats>,
}

type Built = (Vec<Figure>, Vec<Attendance>, Vec<Recruit>, Option<crate::modmail::TicketStats>);

/// Noms affichés pour la page HTML.
fn for_page(ctx: &Ctx, guild_id: Id<GuildMarker>, data: StatsData) -> Built {
    let name = |user_id: &str| {
        user_id
            .parse()
            .ok()
            .and_then(|id| members::display_name(ctx, guild_id, id))
            .unwrap_or_else(|| user_id.to_owned())
    };
    let figures = data.figures.into_iter().map(|f| Figure { label: f.label, value: f.value, hint: f.hint }).collect();
    let attendance = data
        .attendance
        .into_iter()
        .map(|e| Attendance {
            name: name(&e.user_id),
            present: e.present,
            absent: e.absent,
            retard: e.retard,
            percent: e.percent,
        })
        .collect();
    let recruits = data
        .recruits
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
    (figures, attendance, recruits, data.tickets)
}

pub(crate) async fn build(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<StatsData> {
    let gid = guild_id.to_string();

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
    let session = recrues::service::current_session(&ctx.db, &gid).await?;
    let recruits = session.map(|s| s.entries).unwrap_or_default();

    let figure = |key, label, value: String, hint| FigureData { key, label, value, hint };
    let figures = vec![
        figure(
            "presences",
            "Présences clôturées",
            presences_total.to_string(),
            Some(format!("{presences_month} ces 30 derniers jours")),
        ),
        figure("attendance", "Taux de présence moyen", format!("{average} %"), None),
        figure("absences", "Absences en cours", absences.to_string(), None),
        figure("recruits", "Recrues en essai", recruits.len().to_string(), None),
        figure("transactions", "Transactions en cours", transactions.to_string(), None),
        figure("vehicles", "Véhicules enregistrés", vehicles.to_string(), None),
    ];

    let tickets = match &ctx.modmail {
        Some(modmail) if modmail.config.main_server_ids.contains(&guild_id) => Some(modmail.ticket_stats(ctx).await?),
        _ => None,
    };

    Ok(StatsData { figures, attendance: leaderboard, recruits, tickets })
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
            links: vec![
                ("Transactions", "/stats/123/transactions".into()),
                ("Journal du staff", "/stats/123/journal".into()),
            ],
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
        assert!(html.contains("/stats/123/transactions"));
        if let Ok(path) = std::env::var("STATS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, crate::web::ASSET_VERSION),
                &format!("<style>{}</style>", crate::web::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
