//! Liste et recherche des tickets (staff connecté uniquement).

use super::{models::thread_status, store, util::parse_db_time};
use crate::{
    context::Context,
    web::{
        MaybeViewer,
        pages::{self, Layout},
    },
};
use askama::Template;
use axum::{
    extract::{Query, State},
    http::Uri,
    response::Response,
};
use serde::Deserialize;

const PAGE_SIZE: i64 = 25;

struct Row {
    href: String,
    number: i64,
    user_name: String,
    user_id: String,
    status_label: &'static str,
    status_classes: &'static str,
    opened_iso: String,
    opened_display: String,
    exchanges: i64,
    claimed_by: Option<String>,
}

struct Filter {
    label: &'static str,
    value: &'static str,
    active: bool,
}

#[derive(Template)]
#[template(path = "tickets.html")]
struct TicketsPage {
    layout: Layout,
    query: String,
    filters: Vec<Filter>,
    rows: Vec<Row>,
    total: i64,
    page: i64,
    pages: i64,
    previous_href: Option<String>,
    next_href: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    q: Option<String>,
    status: Option<String>,
    page: Option<i64>,
}

pub fn status_badge(status: i64) -> (&'static str, &'static str) {
    match status {
        thread_status::OPEN => ("Ouvert", "bg-staff-soft text-staff"),
        thread_status::SUSPENDED => ("Suspendu", "bg-internal-soft text-internal"),
        _ => ("Fermé", "bg-system-soft text-system"),
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub async fn list(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Query(query): Query<ListQuery>,
) -> Response {
    let path = uri.path_and_query().map_or("/logs", |p| p.as_str());
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, path).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    if !access.tickets {
        return pages::forbidden(&ctx, &viewer, &access);
    }
    let Some(modmail) = ctx.modmail.as_ref() else {
        return pages::not_found().await;
    };

    let text = query.q.unwrap_or_default();
    let status_value = query.status.unwrap_or_default();
    let status = match status_value.as_str() {
        "open" => Some(thread_status::OPEN),
        "closed" => Some(thread_status::CLOSED),
        "suspended" => Some(thread_status::SUSPENDED),
        _ => None,
    };
    let page = query.page.unwrap_or(1).max(1);
    let (threads, total) =
        match store::search_threads(&modmail.db, &text, status, PAGE_SIZE, (page - 1) * PAGE_SIZE).await {
            Ok(found) => found,
            Err(error) => {
                tracing::error!(?error, "recherche de tickets");
                return pages::message_page(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Erreur",
                    "La recherche a échoué.",
                );
            }
        };
    let pages_count = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
    let href = |page: i64| format!("/logs?q={}&status={}&page={page}", encode(&text), encode(&status_value));

    let ids: Vec<String> = threads.iter().map(|t| t.id.clone()).collect();
    let mut claims = super::staff::claims_of(&ctx.db, &ids).await.unwrap_or_default();
    let rows = threads
        .into_iter()
        .map(|t| {
            let (status_label, status_classes) = status_badge(t.status);
            let opened = parse_db_time(&t.created_at);
            Row {
                href: format!("/logs/{}", t.id),
                number: t.thread_number.unwrap_or_default(),
                user_name: t.user_name,
                user_id: t.user_id,
                status_label,
                status_classes,
                opened_iso: opened.map(|d| d.to_rfc3339()).unwrap_or_default(),
                opened_display: opened.map(|d| d.format("%d/%m/%Y %H:%M UTC").to_string()).unwrap_or_default(),
                exchanges: t.exchanges,
                claimed_by: claims.remove(&t.id).map(|c| c.staff_name),
            }
        })
        .collect();

    let filters = [("Tous", ""), ("Ouverts", "open"), ("Suspendus", "suspended"), ("Fermés", "closed")]
        .into_iter()
        .map(|(label, value)| Filter { label, value, active: status_value == value })
        .collect();

    let previous_href = (page > 1).then(|| href(page - 1));
    let next_href = (page < pages_count).then(|| href(page + 1));
    pages::render(&TicketsPage {
        layout: Layout::new(&ctx, "Tickets", Some(&viewer), &access, "/logs"),
        query: text,
        filters,
        rows,
        total,
        page,
        pages: pages_count,
        previous_href,
        next_href,
    })
}

#[derive(Template)]
#[template(path = "staff.html")]
struct StaffPage {
    layout: Layout,
    tickets: super::TicketStats,
}

pub async fn staff(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri) -> Response {
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, uri.path()).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    if !access.tickets {
        return pages::forbidden(&ctx, &viewer, &access);
    }
    let Some(modmail) = ctx.modmail.as_ref() else {
        return pages::not_found().await;
    };
    match modmail.ticket_stats(&ctx).await {
        Ok(tickets) => pages::render(&StaffPage {
            layout: Layout::new(&ctx, "Statistiques du staff", Some(&viewer), &access, "/logs"),
            tickets,
        }),
        Err(error) => {
            tracing::error!(?error, "statistiques du staff");
            pages::message_page(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Statistiques indisponibles.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tickets_page() {
        let row = |number: i64, user: &str, status: i64, exchanges: i64, claimed: Option<&str>| {
            let (status_label, status_classes) = status_badge(status);
            Row {
                href: format!("/logs/{number}"),
                number,
                user_name: user.into(),
                user_id: "106391128718245888".into(),
                status_label,
                status_classes,
                opened_iso: "2026-10-16T21:40:00+00:00".into(),
                opened_display: "16/10/2026 21:40 UTC".into(),
                exchanges,
                claimed_by: claimed.map(Into::into),
            }
        };
        let page = TicketsPage {
            layout: Layout::bare("Tickets"),
            query: String::new(),
            filters: vec![
                Filter { label: "Tous", value: "", active: true },
                Filter { label: "Ouverts", value: "open", active: false },
            ],
            rows: vec![
                row(44, "Lina", thread_status::OPEN, 3, None),
                row(43, "Hugo", thread_status::SUSPENDED, 12, Some("Marco")),
                row(42, "Elodie", thread_status::CLOSED, 7, Some("Golmon")),
            ],
            total: 3,
            page: 1,
            pages: 1,
            previous_href: None,
            next_href: None,
        };
        let html = page.render().unwrap();
        assert!(html.contains("Elodie") && html.contains("🙋 Marco") && html.contains("Suspendu"));
        if let Ok(path) = std::env::var("TICKETS_PREVIEW_PATH") {
            let html = html.replace(
                &format!(r#"<link rel="stylesheet" href="/assets/app.css?v={}">"#, crate::web::ASSET_VERSION),
                &format!("<style>{}</style>", crate::web::CSS),
            );
            std::fs::write(path, html).unwrap();
        }
    }
}
