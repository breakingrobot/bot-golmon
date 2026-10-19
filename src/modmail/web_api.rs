//! Données des tickets pour l'API JSON du site (`/api/tickets`, `/api/logs`) :
//! mêmes requêtes et même mise en forme des messages que les pages HTML.

use super::{
    formatters::LogOptions,
    models::{message_type, thread_status},
    staff, store,
    util::parse_db_time,
    web::build_entries,
};
use crate::{
    context::Ctx,
    web::{
        self,
        api::{
            Page, iso,
            tickets::{LogCounts, LogEdit, LogEntry, LogFile, TicketLog, TicketRow, TicketUser},
        },
    },
};
use anyhow::Result;

pub(crate) use super::web::flag;

pub const PAGE_SIZE: i64 = 25;

/// Statut d'un ticket dans l'API.
pub fn status_key(status: i64) -> &'static str {
    match status {
        thread_status::OPEN => "OPEN",
        thread_status::SUSPENDED => "SUSPENDED",
        _ => "CLOSED",
    }
}

/// Filtre de statut : `OPEN`, `CLOSED`, `SUSPENDED` (ou en minuscules).
pub fn parse_status(raw: &str) -> Option<i64> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "open" => Some(thread_status::OPEN),
        "closed" => Some(thread_status::CLOSED),
        "suspended" => Some(thread_status::SUSPENDED),
        _ => None,
    }
}

/// Recherche de tickets ; `None` sans module ticket.
pub async fn search(ctx: &Ctx, query: &str, status: Option<i64>, page: i64) -> Result<Option<Page<TicketRow>>> {
    let Some(modmail) = ctx.modmail.as_ref() else {
        return Ok(None);
    };
    let (_, total) = store::search_threads(&modmail.db, query, status, 0, 0).await?;
    let (page, offset) = web::api::clamp_page(page, total, PAGE_SIZE);
    let (threads, total) = store::search_threads(&modmail.db, query, status, PAGE_SIZE, offset).await?;
    let ids: Vec<String> = threads.iter().map(|t| t.id.clone()).collect();
    let mut claims = staff::claims_of(&ctx.db, &ids).await.unwrap_or_default();
    let items = threads
        .into_iter()
        .map(|t| TicketRow {
            number: t.thread_number.unwrap_or_default(),
            user: TicketUser { id: t.user_id, name: t.user_name },
            status: status_key(t.status),
            opened_at: parse_db_time(&t.created_at).map(iso).unwrap_or_default(),
            exchanges: t.exchanges,
            claimed_by: claims.remove(&t.id).map(|c| c.staff_name),
            id: t.id,
        })
        .collect();
    Ok(Some(Page { items, total, page, page_size: PAGE_SIZE }))
}

/// Log d'un ticket ; `None` : module absent ou ticket introuvable.
pub async fn log(ctx: &Ctx, thread_id: &str, simple: bool) -> Result<Option<TicketLog>> {
    let Some(modmail) = ctx.modmail.as_ref() else {
        return Ok(None);
    };
    let Some(thread) = store::find_by_id(&modmail.db, thread_id).await? else {
        return Ok(None);
    };
    let messages = store::messages(&modmail.db, &thread.id).await?;
    let count = |kind| messages.iter().filter(|m| m.message_type == kind).count();
    // Identifiants techniques toujours fournis : l'interface choisit de les montrer.
    let options = LogOptions { simple, verbose: true };
    let entries = build_entries(&messages, options, &web::CacheResolver(ctx))
        .into_iter()
        .map(|entry| LogEntry {
            id: entry.id.to_string(),
            kind: entry.tone.key(),
            new_day: entry.day.is_some(),
            author: entry.author,
            role: entry.role,
            badge: Some(entry.badge.to_owned()),
            number: entry.number,
            at: entry.created.map(iso).unwrap_or_default(),
            body_html: entry.body_html,
            monospace: entry.monospace,
            edit: entry.edit.map(|e| LogEdit { before_html: e.before, after_html: e.after }),
            images: entry.images,
            files: entry.files.into_iter().map(|f| LogFile { url: f.url, name: f.name }).collect(),
            embeds: entry.embeds,
            technical: entry.verbose_ids,
        })
        .collect();
    Ok(Some(TicketLog {
        number: thread.number(),
        user: TicketUser { id: thread.user_id.clone(), name: thread.user_name.clone() },
        status: status_key(thread.status),
        opened_at: parse_db_time(&thread.created_at).map(iso).unwrap_or_default(),
        counts: LogCounts {
            from_user: count(message_type::FROM_USER),
            to_user: count(message_type::TO_USER),
            internal: count(message_type::CHAT),
        },
        raw_url: format!("/logs/{}?raw=1", thread.id),
        entries,
    }))
}

/// Statistiques du staff ; `None` sans module ticket.
pub async fn staff_stats(ctx: &Ctx) -> Result<Option<super::TicketStats>> {
    match ctx.modmail.as_ref() {
        Some(modmail) => Ok(Some(modmail.ticket_stats(ctx).await?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_statuses() {
        assert_eq!(status_key(thread_status::OPEN), "OPEN");
        assert_eq!(status_key(thread_status::SUSPENDED), "SUSPENDED");
        assert_eq!(status_key(thread_status::CLOSED), "CLOSED");
        assert_eq!(parse_status("Closed"), Some(thread_status::CLOSED));
        assert_eq!(parse_status("x"), None);
    }
}
