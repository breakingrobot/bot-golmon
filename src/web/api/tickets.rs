//! Tickets (module ticket) : liste, statistiques du staff et logs.

use super::{ApiError, ApiQuery, ApiResult, Session, json, page_number};
use crate::{
    context::Context,
    modmail::web_api,
    web::{Access, MaybeViewer, auth},
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = TicketStaffActivity)]
pub struct StaffActivity {
    pub name: String,
    pub replies: i64,
    pub tickets: i64,
    pub claims: i64,
    pub first_responses: i64,
    #[schema(required = true)]
    pub average_first_response: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketStats {
    pub open: i64,
    pub opened: i64,
    pub closed: i64,
    pub unanswered: i64,
    #[schema(required = true)]
    pub median_first_response: Option<String>,
    pub staff: Vec<StaffActivity>,
}

pub fn ticket_stats(stats: crate::modmail::TicketStats) -> TicketStats {
    TicketStats {
        open: stats.open,
        opened: stats.opened,
        closed: stats.closed,
        unanswered: stats.unanswered,
        median_first_response: stats.median_first_response,
        staff: stats
            .staff
            .into_iter()
            .map(|s| StaffActivity {
                name: s.name,
                replies: s.replies,
                tickets: s.tickets,
                claims: s.claims,
                first_responses: s.first_responses,
                average_first_response: s.average_first_response,
            })
            .collect(),
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketUser {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketRow {
    pub id: String,
    pub number: i64,
    pub user: TicketUser,
    #[schema(value_type = super::openapi::TicketStatus)]
    pub status: &'static str,
    pub opened_at: String,
    pub exchanges: i64,
    #[schema(required = true)]
    pub claimed_by: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogCounts {
    pub from_user: usize,
    pub to_user: usize,
    pub internal: usize,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogEdit {
    pub before_html: String,
    pub after_html: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogFile {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    #[schema(value_type = super::openapi::LogKind)]
    pub kind: &'static str,
    pub new_day: bool,
    pub author: String,
    #[schema(required = true)]
    pub role: Option<String>,
    #[schema(required = true)]
    pub badge: Option<String>,
    #[schema(required = true)]
    pub number: Option<i64>,
    pub at: String,
    pub body_html: String,
    pub monospace: bool,
    #[schema(required = true)]
    pub edit: Option<LogEdit>,
    pub images: Vec<String>,
    pub files: Vec<LogFile>,
    #[schema(required = true)]
    pub embeds: Option<String>,
    #[schema(required = true)]
    pub technical: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketLog {
    pub number: i64,
    pub user: TicketUser,
    #[schema(value_type = super::openapi::TicketStatus)]
    pub status: &'static str,
    pub opened_at: String,
    pub counts: LogCounts,
    pub raw_url: String,
    pub entries: Vec<LogEntry>,
}

fn require_staff(session: &Session) -> ApiResult<()> {
    if session.access.tickets { Ok(()) } else { Err(ApiError::forbidden("Réservé au staff des tickets.")) }
}

fn no_module() -> ApiError {
    ApiError::not_found("Le module ticket n'est pas configuré.")
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TicketQuery {
    /// Identifiant ou nom de l'utilisateur, numéro de ticket ou texte d'un message.
    q: Option<String>,
    /// `OPEN`, `CLOSED` ou `SUSPENDED`.
    status: Option<String>,
    /// Page (1 = première).
    page: Option<String>,
}

/// Recherche de tickets (staff des tickets).
#[utoipa::path(
    get, path = "/api/tickets", tag = "tickets", operation_id = "getTickets",
    params(TicketQuery),
    responses(
        (status = 200, description = "Tickets", body = super::openapi::TicketPage),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn list(State(ctx): State<Context>, session: Session, ApiQuery(query): ApiQuery<TicketQuery>) -> ApiResult {
    require_staff(&session)?;
    let status = query.status.as_deref().and_then(web_api::parse_status);
    let page = page_number(query.page.as_deref());
    let found = web_api::search(&ctx, query.q.as_deref().unwrap_or_default(), status, page).await?;
    Ok(json(&found.ok_or_else(no_module)?))
}

/// Statistiques du staff des tickets (30 derniers jours).
#[utoipa::path(
    get, path = "/api/tickets/staff", tag = "tickets", operation_id = "getTicketStaff",
    responses(
        (status = 200, description = "Statistiques", body = TicketStats),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn staff(State(ctx): State<Context>, session: Session) -> ApiResult {
    require_staff(&session)?;
    let stats = web_api::staff_stats(&ctx).await?.ok_or_else(no_module)?;
    Ok(json(&ticket_stats(stats)))
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LogQuery {
    /// `1` : échanges seulement (sans commandes ni messages du bot).
    simple: Option<String>,
}

/// Staff des tickets ; sans connexion configurée, lisible par son lien non
/// devinable (comme la page de log et modmailbot).
/// Log d'un ticket : staff des tickets, ou lien seul sans connexion configurée.
#[utoipa::path(
    get, path = "/api/logs/{thread}", tag = "tickets", operation_id = "getTicketLog",
    params(("thread" = String, Path, description = "Identifiant du ticket"), LogQuery),
    responses(
        (status = 200, description = "Log", body = TicketLog),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
    )
)]
pub async fn log(
    State(ctx): State<Context>,
    MaybeViewer(viewer): MaybeViewer,
    Path(thread): Path<String>,
    ApiQuery(query): ApiQuery<LogQuery>,
) -> ApiResult {
    if auth::enabled(&ctx) {
        let Some(viewer) = viewer else {
            return Err(ApiError::unauthorized(&ctx));
        };
        if !Access::of(&ctx, viewer.id).await.tickets {
            return Err(ApiError::forbidden("Réservé au staff des tickets."));
        }
    }
    let simple = web_api::flag(query.simple.as_ref());
    let log = web_api::log(&ctx, thread.trim(), simple).await?;
    Ok(json(&log.ok_or_else(|| ApiError::not_found("Ticket introuvable."))?))
}
