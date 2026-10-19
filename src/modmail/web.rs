//! Serveur web du module ticket : logs (HTML lisible ou texte brut au format
//! modmailbot), pièces jointes stockées localement, politique de confidentialité.

use super::{
    formatters::{LogOptions, format_log, shown_in_simple_view, summarise_embeds},
    models::{Thread, ThreadMessage, message_type, thread_status},
    store,
    util::parse_db_time,
};
use crate::{
    context::Context,
    web::{
        self, ASSET_VERSION, MaybeViewer,
        markdown::{self, Resolver},
        pages::Layout,
    },
};
use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Datelike, Utc};
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

/// Style d'un message selon son type. Les classes sont écrites en entier : le
/// compilateur Tailwind les lit ici (`@source`).
#[derive(Clone, Copy)]
pub(crate) enum Tone {
    User,
    Staff,
    Internal,
    Command,
    System,
    Danger,
}

impl Tone {
    /// Clé du type de message dans l'API (`LogEntry.kind`).
    pub fn key(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Staff => "staff",
            Self::Internal => "internal",
            Self::Command => "command",
            Self::System => "system",
            Self::Danger => "danger",
        }
    }

    pub fn card(self) -> &'static str {
        match self {
            Self::User => "border-line border-l-4 border-l-user",
            Self::Staff => "border-line border-l-4 border-l-staff",
            Self::Internal => "border-internal/40 bg-internal-soft/40",
            Self::Command => "border-dashed border-line",
            Self::System => "border-line bg-surface-sunken",
            Self::Danger => "border-danger/40",
        }
    }

    pub fn avatar(self) -> &'static str {
        match self {
            Self::User => "bg-user-soft text-user",
            Self::Staff => "bg-staff-soft text-staff",
            Self::Internal => "bg-internal-soft text-internal",
            Self::Command | Self::System => "bg-system-soft text-system",
            Self::Danger => "bg-danger-soft text-danger",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::User => "bg-user-soft text-user",
            Self::Staff => "bg-staff-soft text-staff",
            Self::Internal => "bg-internal-soft text-internal",
            Self::Command | Self::System => "bg-system-soft text-system",
            Self::Danger => "bg-danger-soft text-danger",
        }
    }
}

pub(crate) struct FileLink {
    pub(crate) url: String,
    pub(crate) name: String,
}

pub(crate) struct Edit {
    pub(crate) before: String,
    pub(crate) after: String,
}

/// Message du log, mis en forme : commun à la page HTML et à l'API.
pub(crate) struct Entry {
    pub(crate) id: i64,
    /// Premier message d'un nouveau jour : libellé du jour.
    pub(crate) day: Option<String>,
    pub(crate) tone: Tone,
    pub(crate) initial: char,
    pub(crate) author: String,
    pub(crate) role: Option<String>,
    pub(crate) badge: &'static str,
    pub(crate) number: Option<i64>,
    pub(crate) created: Option<DateTime<Utc>>,
    pub(crate) time_iso: String,
    pub(crate) time_display: String,
    pub(crate) body_html: String,
    pub(crate) monospace: bool,
    pub(crate) edit: Option<Edit>,
    pub(crate) images: Vec<String>,
    pub(crate) files: Vec<FileLink>,
    pub(crate) embeds: Option<String>,
    pub(crate) verbose_ids: Option<String>,
}

struct View {
    label: &'static str,
    href: String,
    active: bool,
}

#[derive(Template)]
#[template(path = "logs.html")]
struct LogPage {
    layout: Layout,
    thread_number: i64,
    user_name: String,
    user_id: String,
    status_label: &'static str,
    status_classes: &'static str,
    opened_iso: String,
    opened_display: String,
    count_from_user: usize,
    count_to_user: usize,
    count_chat: usize,
    views: Vec<View>,
    raw_href: String,
    entries: Vec<Entry>,
}

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

fn day_label(date: &DateTime<Utc>) -> String {
    format!("{} {} {}", date.day(), MONTHS[date.month0() as usize], date.year())
}

fn is_image(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default().to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif"].iter().any(|ext| path.ends_with(ext))
}

fn file_name(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(url).to_owned()
}

fn entry_for(message: &ThreadMessage, verbose: bool, resolver: &dyn Resolver) -> Entry {
    let render_body = |text: &str| markdown::render(text, resolver);
    let (tone, badge) = match message.message_type {
        message_type::FROM_USER => (Tone::User, "Utilisateur"),
        message_type::TO_USER if message.is_anonymous != 0 => (Tone::Staff, "Staff · anonyme"),
        message_type::TO_USER => (Tone::Staff, "Staff"),
        message_type::CHAT => (Tone::Internal, "Note interne"),
        message_type::COMMAND => (Tone::Command, "Commande"),
        message_type::SYSTEM_TO_USER => (Tone::System, "Bot → utilisateur"),
        message_type::REPLY_EDITED => (Tone::System, "Réponse modifiée"),
        message_type::REPLY_DELETED => (Tone::Danger, "Réponse supprimée"),
        message_type::LEGACY => (Tone::System, "Ancien format"),
        _ => (Tone::System, "Bot"),
    };

    let created = parse_db_time(&message.created_at);
    let mut author = if message.user_name.is_empty() { "Modmail".to_owned() } else { message.user_name.clone() };
    let mut body = message.body().to_owned();
    let mut edit = None;
    match message.message_type {
        message_type::REPLY_EDITED => {
            let original = message.original().unwrap_or_default();
            author = original.user_name.clone();
            let new_body =
                message.metadata_value("newBody").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            edit = Some(Edit { before: render_body(original.body()), after: render_body(&new_body) });
        }
        message_type::REPLY_DELETED => {
            let original = message.original().unwrap_or_default();
            author = original.user_name.clone();
            body = original.body().to_owned();
        }
        _ => {}
    }

    let (images, files): (Vec<String>, Vec<String>) =
        message.attachment_links().into_iter().partition(|url| is_image(url));
    let embeds = [
        summarise_embeds(message.metadata_value("embeds").as_ref(), "Embed envoyé"),
        summarise_embeds(message.metadata_value("forwardedEmbeds").as_ref(), "Embed transféré"),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    let verbose_ids = verbose
        .then(|| {
            let mut parts = Vec::new();
            if let Some(id) = &message.dm_channel_id {
                parts.push(format!("DM CHA {id}"));
            }
            if let Some(id) = &message.dm_message_id {
                parts.push(format!("DM MSG {id}"));
            }
            parts.join(" · ")
        })
        .filter(|s| !s.is_empty());

    Entry {
        id: message.id,
        day: None,
        tone,
        initial: author.chars().find(|c| c.is_alphanumeric()).map_or('•', |c| c.to_uppercase().next().unwrap_or(c)),
        role: (message.message_type == message_type::TO_USER).then(|| message.role_name.clone()).flatten(),
        author,
        badge,
        number: if matches!(message.message_type, message_type::TO_USER) { message.message_number } else { None },
        created,
        time_iso: created.map(|d| d.to_rfc3339()).unwrap_or_default(),
        time_display: created.map(|d| d.format("%H:%M").to_string()).unwrap_or_default(),
        body_html: render_body(&body),
        monospace: matches!(message.message_type, message_type::COMMAND | message_type::LEGACY),
        edit,
        images,
        files: files.into_iter().map(|url| FileLink { name: file_name(&url), url }).collect(),
        embeds: (!embeds.is_empty()).then_some(embeds),
        verbose_ids,
    }
}

/// Messages du log mis en forme, avec un séparateur au premier message de chaque jour.
pub(crate) fn build_entries(messages: &[ThreadMessage], options: LogOptions, resolver: &dyn Resolver) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut last_day = None;
    for message in messages.iter().filter(|m| !options.simple || shown_in_simple_view(m)) {
        let mut entry = entry_for(message, options.verbose, resolver);
        if let Some(date) = parse_db_time(&message.created_at) {
            let day = day_label(&date);
            if last_day.as_ref() != Some(&day) {
                entry.day = Some(day.clone());
                last_day = Some(day);
            }
        }
        entries.push(entry);
    }
    entries
}

fn render_page(
    layout: Layout,
    thread: &Thread,
    messages: &[ThreadMessage],
    options: LogOptions,
    resolver: &dyn Resolver,
) -> Result<String, askama::Error> {
    let count = |kind| messages.iter().filter(|m| m.message_type == kind).count();
    let opened = parse_db_time(&thread.created_at);
    let (status_label, status_classes) = match thread.status {
        thread_status::OPEN => ("Ouvert", "bg-staff-soft text-staff"),
        thread_status::SUSPENDED => ("Suspendu", "bg-internal-soft text-internal"),
        _ => ("Fermé", "bg-system-soft text-system"),
    };
    let entries = build_entries(messages, options, resolver);

    let base = format!("/logs/{}", thread.id);
    LogPage {
        layout,
        thread_number: thread.number(),
        user_name: thread.user_name.clone(),
        user_id: thread.user_id.clone(),
        status_label,
        status_classes,
        opened_iso: opened.map(|d| d.to_rfc3339()).unwrap_or_default(),
        opened_display: opened.map(|d| d.format("%d/%m/%Y à %H:%M UTC").to_string()).unwrap_or_default(),
        count_from_user: count(message_type::FROM_USER),
        count_to_user: count(message_type::TO_USER),
        count_chat: count(message_type::CHAT),
        views: vec![
            View { label: "Tout", href: base.clone(), active: !options.simple && !options.verbose },
            View { label: "Échanges seulement", href: format!("{base}?simple=1"), active: options.simple },
            View {
                label: "Détails techniques",
                href: format!("{base}?verbose=1"),
                active: options.verbose && !options.simple,
            },
        ],
        raw_href: format!("{base}?raw=1"),
        entries,
    }
    .render()
}

#[derive(Deserialize)]
struct LogQuery {
    simple: Option<String>,
    verbose: Option<String>,
    raw: Option<String>,
}

pub(crate) fn flag(value: Option<&String>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0" && v != "false")
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Page Not Found").into_response()
}

async fn serve_log(
    State(ctx): State<Context>,
    viewer: MaybeViewer,
    uri: Uri,
    Path(thread_id): Path<String>,
    Query(query): Query<LogQuery>,
) -> Response {
    let Some(modmail) = ctx.modmail.as_ref() else {
        return not_found();
    };
    // Connexion configurée : logs réservés au staff. Sinon, lien non devinable (modmailbot).
    let mut access = None;
    if web::auth::enabled(&ctx) {
        let path = uri.path_and_query().map_or("/logs", |p| p.as_str());
        let (viewer, granted) = match web::pages::require_viewer(&ctx, viewer, path).await {
            Ok(found) => found,
            Err(response) => return *response,
        };
        if !granted.tickets {
            return web::pages::forbidden(&ctx, &viewer, &granted);
        }
        access = Some((viewer, granted));
    }
    let Ok(Some(thread)) = store::find_by_id(&modmail.db, &thread_id).await else {
        return not_found();
    };
    let Ok(mut messages) = store::messages(&modmail.db, &thread.id).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
    };
    let options = LogOptions { simple: flag(query.simple.as_ref()), verbose: flag(query.verbose.as_ref()) };

    if flag(query.raw.as_ref()) {
        // Format texte exact de modmailbot, embeds résumés dans le corps.
        for message in &mut messages {
            let extra = [
                summarise_embeds(message.metadata_value("embeds").as_ref(), "User sent an embed"),
                summarise_embeds(message.metadata_value("forwardedEmbeds").as_ref(), "User forwarded an embed"),
            ];
            if extra.iter().any(|s| !s.is_empty()) {
                let body = std::iter::once(message.body().to_owned())
                    .chain(extra)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                message.body = Some(body);
            }
        }
        let text = format_log(&thread, &messages, options);
        return ([(header::CONTENT_TYPE, "text/plain; charset=UTF-8")], text).into_response();
    }

    let title = format!("Ticket #{} — {}", thread.number(), thread.user_name);
    let layout = match &access {
        Some((viewer, granted)) => Layout::new(&ctx, title, Some(viewer), granted, "/logs"),
        None => Layout::bare(&title),
    };
    match render_page(layout, &thread, &messages, options, &web::CacheResolver(&ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(error) => {
            tracing::error!(%error, "rendu de la page de log impossible");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

async fn serve_attachment(
    State(ctx): State<Context>,
    Path((attachment_id, filename)): Path<(String, String)>,
) -> Response {
    let Some(modmail) = ctx.modmail.as_ref() else {
        return not_found();
    };
    static FILENAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[0-9a-z._-]+$").unwrap());
    if !attachment_id.bytes().all(|b| b.is_ascii_digit()) || !FILENAME.is_match(&filename) {
        return not_found();
    }
    let Ok(bytes) = tokio::fs::read(modmail.local_attachment_path(&attachment_id)).await else {
        return not_found();
    };
    let ext = filename.rsplit('.').next().unwrap_or("bin").to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain; charset=UTF-8",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, mime)], bytes).into_response()
}

async fn serve_privacy_policy() -> Response {
    match tokio::fs::read_to_string("PRIVACY_POLICY.md").await {
        Ok(text) => Html(format!(
            "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>Privacy policy</title><link rel=\"stylesheet\" href=\"/assets/app.css?v={ASSET_VERSION}\"></head>\
             <body><main class=\"mx-auto max-w-3xl px-4 py-12\"><div class=\"message-body text-ink\">{}</div></main></body></html>",
            markdown::render(&text, &markdown::NoResolver)
        ))
        .into_response(),
        Err(_) => not_found(),
    }
}

pub fn routes() -> Router<Context> {
    Router::new()
        .route("/logs", get(super::web_list::list))
        .route("/logs/staff", get(super::web_list::staff))
        .route("/logs/{thread_id}", get(serve_log))
        .route("/attachments/{attachment_id}/{filename}", get(serve_attachment))
        .route("/privacy-policy", get(serve_privacy_policy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_images() {
        assert!(is_image("https://cdn.discordapp.com/a/b/c.PNG?ex=1"));
        assert!(!is_image("https://cdn.discordapp.com/a/b/c.pdf"));
        assert_eq!(file_name("https://x/y/report.pdf?z=1"), "report.pdf");
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;

    fn message(id: i64, kind: i64, user: &str, body: &str, time: &str) -> ThreadMessage {
        ThreadMessage {
            id,
            thread_id: "t".into(),
            message_type: kind,
            user_name: user.into(),
            user_id: Some("106391128718245888".into()),
            body: Some(body.into()),
            created_at: time.into(),
            ..Default::default()
        }
    }

    #[test]
    fn renders_full_log_page() {
        let thread = Thread {
            id: "3f0c2c1e-0000-4000-8000-000000000000".into(),
            status: thread_status::CLOSED,
            user_id: "106391128718245888".into(),
            user_name: "Elodie".into(),
            channel_id: None,
            created_at: "2026-10-16 21:40:00".into(),
            scheduled_close_at: None,
            scheduled_close_id: None,
            scheduled_close_name: None,
            scheduled_close_silent: None,
            scheduled_suspend_at: None,
            scheduled_suspend_name: None,
            alert_ids: None,
            log_storage_type: Some("local".into()),
            log_storage_data: None,
            thread_number: Some(42),
        };
        let mut staff = message(
            3,
            message_type::TO_USER,
            "Marco",
            "Bonjour ! Peux-tu nous envoyer une capture ? Règlement : https://example.com/regles",
            "2026-10-16 21:43:10",
        );
        staff.role_name = Some("Modération".into());
        staff.message_number = Some(1);
        let mut with_image =
            message(4, message_type::FROM_USER, "Elodie", "Voilà la capture 👇", "2026-10-16 21:45:02");
        with_image.attachments = Some(r#"["https://cdn.discordapp.com/attachments/1/2/capture.png","https://cdn.discordapp.com/attachments/1/3/rapport.pdf"]"#.into());
        let mut edited = message(7, message_type::REPLY_EDITED, "", "", "2026-10-17 09:01:00");
        edited.metadata = Some(r#"{"originalThreadMessage":{"user_name":"Marco","body":"On regarde ça demain.","message_number":2},"newBody":"On regarde ça ce matin, merci de ta patience !"}"#.into());
        let messages = vec![
            message(
                1,
                message_type::SYSTEM,
                "",
                "ACCOUNT AGE **3 years, 2 months**, ID **106391128718245888**\nNICKNAME **Elodie**, JOINED **8 months** ago\n────────────────",
                "2026-10-16 21:40:00",
            ),
            message(
                2,
                message_type::FROM_USER,
                "Elodie",
                "Bonjour, je n'arrive plus à accéder au salon <#123456789012345678> depuis hier soir.",
                "2026-10-16 21:40:01",
            ),
            staff,
            with_image,
            message(
                41,
                message_type::FROM_USER,
                "Elodie",
                "## Récapitulatif\n> Rôle perdu **pendant la maintenance** du <t:1792180800:D>\n> (il y a <t:1792180800:R>)\n- salon <#123456789012345678>\n- pseudo `Elodie#0001`\n  - ~~ancien~~ __nouveau__ ||secret|| <:salut:123456789012345678>\n1. voir [le règlement](https://example.com/regles)\n-# envoyé depuis mobile\n```rust\nfn main() {}\n```",
                "2026-10-16 21:45:30",
            ),
            message(
                5,
                message_type::CHAT,
                "Marco",
                "Vu, c'est le rôle qui a sauté pendant la maintenance. Je regarde demain.",
                "2026-10-16 21:46:30",
            ),
            message(6, message_type::COMMAND, "Marco", "!close 12h", "2026-10-16 21:47:00"),
            edited,
            message(8, message_type::SYSTEM, "", "Closing thread...", "2026-10-17 09:30:00"),
        ];
        let html =
            render_page(Layout::bare("Ticket"), &thread, &messages, LogOptions::default(), &markdown::NoResolver)
                .unwrap();
        assert!(html.contains("Ticket #42"));
        assert!(html.contains("Note interne"));
        assert!(html.contains("<img src=\"https://cdn.discordapp.com/attachments/1/2/capture.png\""));
        assert!(html.contains("rapport.pdf"));
        assert!(html.contains("16 octobre 2026") && html.contains("17 octobre 2026"));

        if let Ok(path) = std::env::var("LOG_PREVIEW_PATH") {
            let css = format!("<style>{}</style>", crate::web::CSS);
            let preview =
                html.replace(&format!(r#"<link rel="stylesheet" href="/assets/app.css?v={ASSET_VERSION}">"#), &css);
            std::fs::write(path, preview).unwrap();
        }
    }
}
