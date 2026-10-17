//! Membres des guildes, lus dans le cache.
//!
//! Avec l'intent GUILD_MEMBERS, le cache est tenu à jour par les événements
//! GUILD_MEMBER_ADD/UPDATE/REMOVE : une seule demande de membres (opcode 8,
//! fortement limité) à l'arrivée de chaque guilde suffit.

use crate::context::Ctx;
use twilight_model::{
    gateway::payload::outgoing::RequestGuildMembers,
    id::{
        Id,
        marker::{GuildMarker, RoleMarker, UserMarker},
    },
};

pub fn request_all_members(ctx: &Ctx, guild_id: Id<GuildMarker>) {
    let Some(sender) = ctx.sender_for(guild_id) else {
        return;
    };
    let request = RequestGuildMembers::builder(guild_id).query("", None);
    if let Err(error) = sender.command(&request) {
        tracing::warn!(%guild_id, %error, "demande des membres impossible");
    }
}

/// Identifiants des membres portant le rôle (cache).
pub fn role_member_ids(ctx: &Ctx, guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> Vec<String> {
    let Some(members) = ctx.cache.guild_members(guild_id) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = members
        .iter()
        .filter(|user_id| ctx.cache.member(guild_id, **user_id).is_some_and(|m| m.roles().contains(&role_id)))
        .map(ToString::to_string)
        .collect();
    ids.sort_unstable();
    ids
}

/// Pseudo affiché : surnom de serveur, sinon nom global, sinon nom d'utilisateur.
pub fn display_name(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Option<String> {
    let nick = ctx.cache.member(guild_id, user_id).and_then(|m| m.nick().map(str::to_owned));
    nick.or_else(|| ctx.cache.user(user_id).map(|u| u.global_name.clone().unwrap_or_else(|| u.name.clone())))
}

/// Pseudo affiché, en interrogeant l'API si le membre n'est pas en cache.
pub async fn fetch_display_name(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Option<String> {
    if let Some(name) = display_name(ctx, guild_id, user_id) {
        return Some(name);
    }
    let member = ctx.http.guild_member(guild_id, user_id).await.ok()?.model().await.ok()?;
    Some(member.nick.or(member.user.global_name).unwrap_or(member.user.name))
}

pub fn guild_icon_url(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Option<String> {
    let guild = ctx.cache.guild(guild_id)?;
    let icon = guild.icon()?;
    let ext = if icon.is_animated() { "gif" } else { "png" };
    Some(format!("https://cdn.discordapp.com/icons/{guild_id}/{icon}.{ext}"))
}

pub fn avatar_url(user: &twilight_model::user::User) -> String {
    match user.avatar {
        Some(hash) => {
            let ext = if hash.is_animated() { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/avatars/{}/{hash}.{ext}", user.id)
        }
        None => format!("https://cdn.discordapp.com/embed/avatars/{}.png", (user.id.get() >> 22) % 6),
    }
}
