//! Permissions du serveur (« Gérer le serveur ») : comme `/config permission`.

use super::{ApiError, ApiResult, JsonBody, RoleRef, Session, json, role_ref};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        permissions::{Level, Perm, service},
    },
};
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PermissionRoles)]
pub struct Roles {
    #[schema(required = true)]
    pub member: Option<RoleRef>,
    #[schema(required = true)]
    pub grade: Option<RoleRef>,
    #[schema(required = true)]
    pub lead: Option<RoleRef>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = PermissionAction)]
pub struct ActionLevel {
    #[schema(value_type = super::openapi::PermKey)]
    pub key: &'static str,
    pub module: &'static str,
    pub label: &'static str,
    #[schema(value_type = super::openapi::LevelKey)]
    pub default_level: &'static str,
    #[schema(value_type = super::openapi::LevelKey)]
    pub level: &'static str,
    pub strict: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsResponse {
    pub roles: Roles,
    pub guild_roles: Vec<RoleRef>,
    pub actions: Vec<ActionLevel>,
}

/// Rôles du serveur, le plus haut d'abord (sans @everyone).
pub(super) fn guild_roles(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Vec<RoleRef> {
    let Some(ids) = ctx.cache.guild_roles(guild_id) else {
        return Vec::new();
    };
    let mut roles: Vec<(i64, u64, RoleRef)> = ids
        .iter()
        .filter(|id| id.get() != guild_id.get())
        .filter_map(|id| ctx.cache.role(*id))
        .map(|role| {
            let role = role.resource();
            (
                role.position,
                role.id.get(),
                RoleRef { id: role.id.to_string(), name: role.name.clone(), color: super::role_color(role) },
            )
        })
        .collect();
    roles.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    roles.into_iter().map(|(_, _, role)| role).collect()
}

async fn build(ctx: &Ctx, guild_id: Id<GuildMarker>) -> anyhow::Result<PermissionsResponse> {
    let guild = guild_id.to_string();
    let roles = service::roles(&ctx.db, &guild).await?;
    let levels = service::levels(&ctx.db, &guild).await?;
    let role = |id: &Option<String>| id.as_deref().map(|id| role_ref(ctx, id));
    Ok(PermissionsResponse {
        roles: Roles { member: role(&roles.member), grade: role(&roles.grade), lead: role(&roles.lead) },
        guild_roles: guild_roles(ctx, guild_id),
        actions: Perm::ALL
            .iter()
            .map(|perm| ActionLevel {
                key: perm.key(),
                module: perm.module(),
                label: perm.label(),
                default_level: perm.default_level().as_str(),
                level: levels.get(perm.key()).copied().unwrap_or(perm.default_level()).as_str(),
                strict: perm.strict(),
            })
            .collect(),
    })
}

/// Permissions du serveur (« Gérer le serveur »).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/permissions", tag = "reglages", operation_id = "getPermissions",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Permissions", body = PermissionsResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn show(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    Ok(json(&build(&ctx, guild_id).await?))
}

/// Seuls les champs présents changent ; `null` dans `levels` = niveau par défaut.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionsUpdate {
    /// `null` : retirer le rôle membre.
    #[serde(deserialize_with = "super::double_option")]
    #[schema(value_type = Option<String>)]
    pub member_role_id: Option<Option<String>>,
    /// Clé d'action → niveau ; `null` = revenir au niveau par défaut.
    #[schema(value_type = Option<std::collections::HashMap<String, super::openapi::LevelKey>>)]
    pub levels: Option<BTreeMap<String, Option<String>>>,
}

/// Changements validés.
#[derive(Debug, PartialEq, Eq)]
pub struct Changes {
    /// `Some(None)` : retirer le rôle membre.
    pub member_role: Option<Option<String>>,
    pub levels: Vec<(Perm, Option<Level>)>,
}

/// Vérifie les clés et niveaux ; `role_exists` dit si un rôle est sur le serveur.
pub fn validate(update: PermissionsUpdate, role_exists: impl Fn(&str) -> bool) -> Result<Changes, String> {
    let member_role = match update.member_role_id {
        Some(Some(raw)) => {
            let raw = raw.trim().to_owned();
            if raw.is_empty() {
                Some(None)
            } else if !raw.bytes().all(|b| b.is_ascii_digit()) || !role_exists(&raw) {
                return Err("Rôle inconnu sur ce serveur.".to_owned());
            } else {
                Some(Some(raw))
            }
        }
        Some(None) => Some(None),
        None => None,
    };
    let mut levels = Vec::new();
    for (key, level) in update.levels.unwrap_or_default() {
        let perm = Perm::parse(&key).ok_or_else(|| format!("Action inconnue : « {key} »."))?;
        let level = match level {
            Some(raw) => Some(Level::parse(&raw).ok_or_else(|| format!("Niveau inconnu : « {raw} »."))?),
            None => None,
        };
        levels.push((perm, level));
    }
    Ok(Changes { member_role, levels })
}

/// Résumé pour le journal, comme les options de `/config permission`.
pub fn summary(changes: &Changes) -> String {
    let mut parts = Vec::new();
    match &changes.member_role {
        Some(Some(role)) => parts.push(format!("role_membre : <@&{role}>")),
        Some(None) => parts.push("role_membre : retiré".to_owned()),
        None => {}
    }
    for (perm, level) in &changes.levels {
        let level = level.map_or("défaut", Level::as_str);
        parts.push(format!("action : {}, niveau : {level}", perm.key()));
    }
    parts.join(", ")
}

/// Change le rôle membre et des niveaux, comme `/config permission` (« Gérer le serveur »).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/permissions", tag = "reglages", operation_id = "updatePermissions",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = PermissionsUpdate,
    responses(
        (status = 200, description = "Permissions à jour", body = PermissionsResponse),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn update(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(update): JsonBody<PermissionsUpdate>,
) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    let gid = guild_id.to_string();
    let role_exists = |raw: &str| {
        raw.parse()
            .ok()
            .and_then(Id::new_checked)
            .and_then(|id| ctx.cache.role(id))
            .is_some_and(|role| role.guild_id() == guild_id)
    };
    let changes = validate(update, role_exists).map_err(ApiError::invalid)?;
    if let Some(role) = &changes.member_role {
        service::set_member_role(&ctx.db, &gid, role.as_deref()).await?;
    }
    for (perm, level) in &changes.levels {
        service::set_level(&ctx.db, &gid, *perm, *level).await?;
    }
    let summary = summary(&changes);
    if !summary.is_empty() {
        let actor = session.actor_name(&ctx, guild_id);
        let entry = Entry::new(Action::Config, None, format!("**permission** — {summary}"));
        audit::record_web(&ctx, guild_id, session.viewer.id, &actor, entry).await;
    }
    Ok(json(&build(&ctx, guild_id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_updates() {
        let update: PermissionsUpdate =
            serde_json::from_str(r#"{"memberRoleId":"42","levels":{"contact.ajouter":"grade","coffre.voir":null}}"#)
                .unwrap();
        let changes = validate(update, |id| id == "42").unwrap();
        assert_eq!(changes.member_role, Some(Some("42".into())));
        assert_eq!(changes.levels, vec![(Perm::CoffreVoir, None), (Perm::ContactAjouter, Some(Level::Grade))]);
        assert_eq!(
            summary(&changes),
            "role_membre : <@&42>, action : coffre.voir, niveau : défaut, action : contact.ajouter, niveau : grade"
        );

        let cleared: PermissionsUpdate = serde_json::from_str(r#"{"memberRoleId":null}"#).unwrap();
        assert_eq!(validate(cleared, |_| false).unwrap().member_role, Some(None));
        let untouched: PermissionsUpdate = serde_json::from_str("{}").unwrap();
        assert_eq!(validate(untouched, |_| false).unwrap(), Changes { member_role: None, levels: Vec::new() });

        let unknown_role: PermissionsUpdate = serde_json::from_str(r#"{"memberRoleId":"7"}"#).unwrap();
        assert!(validate(unknown_role, |_| false).is_err());
        let bad_key: PermissionsUpdate = serde_json::from_str(r#"{"levels":{"tout.casser":"grade"}}"#).unwrap();
        assert!(validate(bad_key, |_| true).unwrap_err().contains("Action inconnue"));
        let bad_level: PermissionsUpdate = serde_json::from_str(r#"{"levels":{"coffre.voir":"roi"}}"#).unwrap();
        assert!(validate(bad_level, |_| true).unwrap_err().contains("Niveau inconnu"));
    }
}
