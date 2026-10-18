//! Mise en page commune, accueil et pages d'erreur.

use super::{ASSET_VERSION, Access, MaybeViewer, Viewer, auth};
use crate::context::{Context, Ctx};
use crate::modules::permissions::Perm;
use askama::Template;
use axum::{
    extract::State,
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Redirect, Response},
};

pub struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
    pub active: bool,
}

/// En-tête des pages : navigation selon les droits, personne connectée.
pub struct Layout {
    pub title: String,
    pub nav: Vec<NavLink>,
    pub viewer_name: Option<String>,
    pub login_href: Option<String>,
    pub asset_version: &'static str,
}

impl Layout {
    pub fn new(ctx: &Ctx, title: impl Into<String>, viewer: Option<&Viewer>, access: &Access, current: &str) -> Self {
        let mut nav = Vec::new();
        if access.tickets {
            nav.push(NavLink { label: "Tickets", href: "/logs", active: current == "/logs" });
        }
        let links = [
            (Perm::SiteStats, "Statistiques", "/stats"),
            (Perm::CotisationStats, "Cotisations", "/cotisations"),
            (Perm::ContactNotes, "Contacts", "/contacts"),
        ];
        for (perm, label, href) in links {
            if !access.guilds_with(perm).is_empty() {
                nav.push(NavLink { label, href, active: current == href });
            }
        }
        Self {
            title: title.into(),
            nav,
            viewer_name: viewer.map(|v| v.name.clone()),
            login_href: (viewer.is_none() && auth::enabled(ctx)).then(|| auth::login_href(current)),
            asset_version: ASSET_VERSION,
        }
    }

    pub fn bare(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            nav: Vec::new(),
            viewer_name: None,
            login_href: None,
            asset_version: ASSET_VERSION,
        }
    }
}

pub fn render(template: &impl Template) -> Response {
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(error) => {
            tracing::error!(%error, "rendu de page impossible");
            (StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne").into_response()
        }
    }
}

#[derive(Template)]
#[template(path = "message.html")]
struct MessagePage {
    layout: Layout,
    heading: String,
    text: String,
    action: Option<(String, &'static str)>,
}

pub fn message_page(status: StatusCode, heading: &str, text: &str) -> Response {
    let page =
        MessagePage { layout: Layout::bare(heading), heading: heading.to_owned(), text: text.to_owned(), action: None };
    (status, render(&page)).into_response()
}

pub async fn not_found() -> Response {
    message_page(StatusCode::NOT_FOUND, "Page introuvable", "Cette page n'existe pas ou n'existe plus.")
}

/// Vérifie qu'une personne est connectée ; sinon renvoie vers la connexion
/// (ou explique que la connexion n'est pas configurée).
pub async fn require_viewer(ctx: &Ctx, viewer: MaybeViewer, path: &str) -> Result<(Viewer, Access), Box<Response>> {
    if !auth::enabled(ctx) {
        return Err(Box::new(message_page(
            StatusCode::SERVICE_UNAVAILABLE,
            "Connexion non configurée",
            "Cette page demande une connexion Discord : renseigner DISCORD_CLIENT_SECRET et WEB_URL (voir la documentation).",
        )));
    }
    let Some(viewer) = viewer.0 else {
        return Err(Box::new(Redirect::to(&auth::login_href(path)).into_response()));
    };
    let access = Access::of(ctx, viewer.id).await;
    Ok((viewer, access))
}

pub fn forbidden(ctx: &Ctx, viewer: &Viewer, access: &Access) -> Response {
    let page = MessagePage {
        layout: Layout::new(ctx, "Accès refusé", Some(viewer), access, ""),
        heading: "Accès refusé".to_owned(),
        text: format!(
            "Connecté en tant que {}, mais ce compte n'a pas accès à cette page. Il faut faire partie du staff des tickets, ou avoir le niveau requis par `/config permission` sur Discord.",
            viewer.name
        ),
        action: Some(("/logout".to_owned(), "Changer de compte")),
    };
    (StatusCode::FORBIDDEN, render(&page)).into_response()
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomePage {
    layout: Layout,
    login_enabled: bool,
    logged_in: bool,
    tickets: bool,
    /// Serveurs où le compte a au moins une page : nom et pages (libellé, adresse).
    guilds: Vec<(String, Vec<(&'static str, String)>)>,
}

pub async fn home(State(ctx): State<Context>, viewer: MaybeViewer, uri: Uri) -> Response {
    let access = match &viewer.0 {
        Some(v) => Access::of(&ctx, v.id).await,
        None => Access::default(),
    };
    let page = HomePage {
        layout: Layout::new(&ctx, "Accueil", viewer.0.as_ref(), &access, uri.path()),
        login_enabled: auth::enabled(&ctx),
        logged_in: viewer.0.is_some(),
        tickets: access.tickets,
        guilds: access
            .guilds
            .iter()
            .map(|g| (g.name.clone(), access.pages(g.id)))
            .filter(|(_, pages)| !pages.is_empty())
            .collect(),
    };
    render(&page)
}
