//! Journal du staff (`site.journal`).

use super::{ApiQuery, ApiResult, MemberRef, Members, Page, Session, clamp_page, iso_db, json, page_number};
use crate::{
    context::Context,
    modules::{
        audit::{self, Action},
        permissions::Perm,
    },
    web::{CacheResolver, markdown},
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

const PAGE_SIZE: i64 = 50;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct JournalQuery {
    /// Code d'action (`actions[].key`).
    action: Option<String>,
    /// Page (1 = première).
    page: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub at: String,
    pub actor: MemberRef,
    pub action: String,
    pub label: String,
    #[schema(required = true)]
    pub target: Option<MemberRef>,
    pub details_html: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = JournalAction)]
pub struct ActionChoice {
    pub key: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JournalResponse {
    pub actions: Vec<ActionChoice>,
    #[schema(value_type = super::openapi::JournalEntryPage)]
    pub list: Page<JournalEntry>,
}

/// Journal du staff (`site.journal`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/journal", tag = "journal", operation_id = "getJournal",
    params(("guild" = String, Path, description = "Identifiant du serveur"), JournalQuery),
    responses(
        (status = 200, description = "Journal", body = JournalResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn journal(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    ApiQuery(query): ApiQuery<JournalQuery>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::Journal)?;
    let gid = guild_id.to_string();
    let action = query.action.as_deref().and_then(Action::from_code);
    let code = action.map(Action::code);
    // Premier passage pour le total, puis la page (bornée au nombre de pages).
    let (_, total) = audit::recent(&ctx.db, &gid, code, 0, 0).await?;
    let (page, offset) = clamp_page(page_number(query.page.as_deref()), total, PAGE_SIZE);
    let (rows, total) = audit::recent(&ctx.db, &gid, code, PAGE_SIZE, offset).await?;

    let members = Members::new(&ctx, guild_id);
    let resolver = CacheResolver(&ctx);
    let items = rows
        .into_iter()
        .map(|row| JournalEntry {
            id: row.id.to_string(),
            at: iso_db(row.created_at),
            actor: members.get_or(&row.actor_id, Some(&row.actor_name)),
            label: Action::from_code(&row.action).map_or("Action", Action::label).to_owned(),
            action: row.action,
            target: row.target_id.as_deref().map(|id| members.get(id)),
            details_html: markdown::render(&row.details, &resolver),
        })
        .collect();
    Ok(json(&JournalResponse {
        actions: Action::ALL.iter().map(|a| ActionChoice { key: a.code(), label: a.label() }).collect(),
        list: Page { items, total, page, page_size: PAGE_SIZE },
    }))
}
