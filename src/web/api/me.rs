//! `GET /api/me` : session et droits, toujours 200 (même sans connexion).

use super::{json, login_url};
use crate::{
    context::{Context, Ctx},
    discord::members,
    web::{Access, MaybeViewer, auth},
};
use axum::{extract::State, response::Response};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    pub user: Option<User>,
    pub login_enabled: bool,
    pub login_url: String,
    pub tickets: bool,
    pub guilds: Vec<GuildAccess>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildAccess {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub is_admin: bool,
    pub perms: Vec<&'static str>,
}

pub fn build(ctx: &Ctx, user: Option<User>, access: &Access) -> Me {
    Me {
        user,
        login_enabled: auth::enabled(ctx),
        login_url: login_url(),
        tickets: access.tickets,
        guilds: access
            .guilds
            .iter()
            .map(|g| GuildAccess {
                id: g.id.to_string(),
                name: g.name.clone(),
                icon_url: members::guild_icon_url(ctx, g.id),
                is_admin: g.is_admin(),
                perms: g.perms().into_iter().map(|p| p.key()).collect(),
            })
            .collect(),
    }
}

pub async fn me(State(ctx): State<Context>, MaybeViewer(viewer): MaybeViewer) -> Response {
    // Sans connexion configurée, un vieux cookie ne vaut rien.
    let viewer = viewer.filter(|_| auth::enabled(&ctx));
    let access = match &viewer {
        Some(viewer) => Access::of(&ctx, viewer.id).await,
        None => Access::default(),
    };
    let user = viewer.map(|v| User {
        id: v.id.to_string(),
        avatar_url: ctx.cache.user(v.id).map(|u| members::avatar_url(&u)),
        name: v.name,
    });
    json(&build(&ctx, user, &access))
}
