//! Journal des actions du staff d'un serveur (accès : `site.journal`).

use super::{
    MaybeViewer, markdown,
    pages::{self, Layout},
};
use crate::{
    context::Context,
    discord::members,
    modules::audit::{self, Action},
    modules::permissions::Perm,
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Uri},
    response::Response,
};
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

const PAGE_SIZE: i64 = 50;

struct Row {
    time_iso: String,
    time_display: String,
    actor: String,
    label: &'static str,
    target: Option<String>,
    details_html: String,
}

struct Filter {
    label: &'static str,
    value: &'static str,
    active: bool,
}

#[derive(Template)]
#[template(path = "journal.html")]
struct JournalPage {
    layout: Layout,
    guild_name: String,
    back_href: String,
    filters: Vec<Filter>,
    rows: Vec<Row>,
    total: i64,
    page: i64,
    pages: i64,
    previous_href: Option<String>,
    next_href: Option<String>,
}

#[derive(Deserialize)]
pub struct JournalQuery {
    action: Option<String>,
    page: Option<i64>,
}

pub async fn page(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(guild_id): Path<u64>,
    Query(query): Query<JournalQuery>,
) -> Response {
    let path = uri.path_and_query().map_or("/", |p| p.as_str());
    let (viewer, access) = match pages::require_viewer(&ctx, viewer, path).await {
        Ok(found) => found,
        Err(response) => return *response,
    };
    let Some(guild_id) = Id::<GuildMarker>::new_checked(guild_id) else {
        return pages::not_found().await;
    };
    if !access.can(guild_id, Perm::Journal) {
        return pages::forbidden(&ctx, &viewer, &access);
    }
    let guild_name = access.guild_name(guild_id);

    let action = query.action.as_deref().and_then(Action::from_code);
    let page = query.page.unwrap_or(1).max(1);
    let (entries, total) = match audit::recent(
        &ctx.db,
        &guild_id.to_string(),
        action.map(Action::code),
        PAGE_SIZE,
        (page - 1) * PAGE_SIZE,
    )
    .await
    {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(?error, "lecture du journal du staff");
            return pages::message_page(StatusCode::INTERNAL_SERVER_ERROR, "Erreur", "Journal indisponible.");
        }
    };

    let resolver = super::CacheResolver(&ctx);
    let name = |id: &str| {
        id.parse().ok().and_then(|id| members::display_name(&ctx, guild_id, id)).unwrap_or_else(|| id.to_owned())
    };
    let rows = entries
        .into_iter()
        .map(|entry| Row {
            time_iso: entry.created_at.0.to_rfc3339(),
            time_display: entry.created_at.0.format("%d/%m/%Y %H:%M UTC").to_string(),
            actor: if entry.actor_name.is_empty() { name(&entry.actor_id) } else { entry.actor_name },
            label: Action::from_code(&entry.action).map_or("Action", Action::label),
            target: entry.target_id.as_deref().map(name),
            details_html: markdown::render(&entry.details, &resolver),
        })
        .collect();

    let pages_count = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
    let base = format!("/stats/{guild_id}/journal");
    let href = |page: i64| match action {
        Some(action) => format!("{base}?action={}&page={page}", action.code()),
        None => format!("{base}?page={page}"),
    };
    let filters = std::iter::once(Filter { label: "Toutes les actions", value: "", active: action.is_none() })
        .chain(Action::ALL.into_iter().map(|a| Filter { label: a.label(), value: a.code(), active: action == Some(a) }))
        .collect();

    pages::render(&JournalPage {
        layout: Layout::new(&ctx, format!("Journal · {guild_name}"), Some(&viewer), &access, "/stats"),
        guild_name,
        back_href: format!("/stats/{guild_id}"),
        filters,
        rows,
        total,
        page,
        pages: pages_count,
        previous_href: (page > 1).then(|| href(page - 1)),
        next_href: (page < pages_count).then(|| href(page + 1)),
    })
}
