//! Réglages tenus par les modules : objectif des cotisations, alertes des
//! coffres, état de la configuration. Les structures des modules ont la même
//! forme JSON que celles de la spécification : la conversion passe par serde.

use super::{ApiError, ApiResult, ErrorBody, JsonBody, Members, Session, json, openapi};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry},
        coffres::alerts,
        cotisations::objective,
        permissions::Perm,
    },
    util::time::week_start,
};
use axum::extract::{Path, State};
use serde::{Serialize, de::DeserializeOwned};
use twilight_model::id::{Id, marker::GuildMarker};

/// Même forme JSON des deux côtés : passage par `serde_json::Value`.
fn convert<A: Serialize, B: DeserializeOwned>(value: &A) -> anyhow::Result<B> {
    Ok(serde_json::from_value(serde_json::to_value(value)?)?)
}

fn to_api_objective(value: objective::Objective) -> anyhow::Result<openapi::Objective> {
    convert(&value)
}

async fn record_config(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, module: &str, summary: String) {
    let actor = session.actor_name(ctx, guild_id);
    let entry = Entry::new(Action::Config, None, format!("**{module}** — {summary}"));
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, entry).await;
}

/// Avancement de la semaine en cours, noms résolus pour le site.
pub async fn week_progress(ctx: &Ctx, guild_id: Id<GuildMarker>) -> anyhow::Result<Vec<openapi::ObjectiveProgress>> {
    let rows = objective::objective_progress(ctx, guild_id, week_start(chrono::Utc::now())).await?;
    let members = Members::new(ctx, guild_id);
    Ok(rows.into_iter().map(|row| progress(&members, row)).collect())
}

pub fn progress(members: &Members, row: objective::ProgressRow) -> openapi::ObjectiveProgress {
    openapi::ObjectiveProgress {
        member: members.get(&row.user_id),
        amount: row.amount,
        target: row.target,
        reached: row.reached,
        exempt: row.exempt,
    }
}

/// Objectif du serveur au format du site (`None` sans objectif).
pub async fn current_objective(ctx: &Ctx, guild: &str) -> anyhow::Result<Option<openapi::Objective>> {
    objective::get_objective(&ctx.db, guild).await?.map(to_api_objective).transpose()
}

/// Avancement d'un membre cette semaine, `None` sans objectif.
pub async fn member_week(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    user_id: &str,
) -> anyhow::Result<Option<openapi::ObjectiveProgress>> {
    let guild = guild_id.to_string();
    let Some(goal) = objective::get_objective(&ctx.db, &guild).await? else {
        return Ok(None);
    };
    let row = objective::member_progress(&ctx.db, &guild, user_id, week_start(chrono::Utc::now()), &goal).await?;
    Ok(Some(progress(&Members::new(ctx, guild_id), row)))
}

/// Objectif hebdomadaire des cotisations (lecture `cotisation.stats`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/cotisations/objective", tag = "cotisations", operation_id = "getCotisationObjective",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Objectif, `null` sans objectif", body = Option<openapi::Objective>),
        (status = 401, description = "Pas connecté", body = ErrorBody),
        (status = 403, description = "Droit insuffisant", body = ErrorBody),
    )
)]
pub async fn objective_get(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CotisationStats)?;
    let objective = objective::get_objective(&ctx.db, &guild_id.to_string()).await?;
    Ok(json(&objective.map(to_api_objective).transpose()?))
}

/// Règle l'objectif hebdomadaire (« Gérer le serveur »).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/cotisations/objective", tag = "cotisations", operation_id = "updateCotisationObjective",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = Option<openapi::Objective>,
    responses(
        (status = 200, description = "Objectif enregistré", body = Option<openapi::Objective>),
        (status = 401, description = "Pas connecté", body = ErrorBody),
        (status = 403, description = "Réservé à « Gérer le serveur »", body = ErrorBody),
        (status = 422, description = "Saisie refusée", body = ErrorBody),
    )
)]
pub async fn objective_put(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(body): JsonBody<Option<openapi::Objective>>,
) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    let gid = guild_id.to_string();
    // `null` ou montant 0 : plus d'objectif.
    let wanted: objective::Objective = match body {
        Some(body) => convert(&body).map_err(|_| ApiError::invalid("Objectif illisible."))?,
        None => objective::Objective { amount: 0, counts: objective::Counts::default(), reminder: false },
    };
    objective::set_objective(&ctx.db, &gid, &wanted).await.map_err(|error| ApiError::invalid(error.to_string()))?;
    let summary = if wanted.amount > 0 {
        format!("objectif hebdomadaire : {}", crate::modules::cotisations::money(wanted.amount))
    } else {
        "objectif hebdomadaire retiré".to_owned()
    };
    record_config(&ctx, &session, guild_id, "cotisation", summary).await;
    crate::modules::cotisations::dashboard::refresh(&ctx, guild_id).await;
    let saved = objective::get_objective(&ctx.db, &gid).await?;
    Ok(json(&saved.map(to_api_objective).transpose()?))
}

/// Alertes des coffres (lecture `coffre.voir`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/coffres/alerts", tag = "coffres", operation_id = "getCoffreAlerts",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Réglage des alertes", body = openapi::CoffreAlerts),
        (status = 401, description = "Pas connecté", body = ErrorBody),
        (status = 403, description = "Droit insuffisant", body = ErrorBody),
    )
)]
pub async fn alerts_get(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require(&guild, Perm::CoffreVoir)?;
    let alerts = alerts::get_alerts(&ctx.db, &guild_id.to_string()).await?;
    Ok(json(&convert::<_, openapi::CoffreAlerts>(&alerts)?))
}

/// Règle les alertes des coffres (« Gérer le serveur »).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/coffres/alerts", tag = "coffres", operation_id = "updateCoffreAlerts",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    request_body = openapi::CoffreAlerts,
    responses(
        (status = 200, description = "Réglage enregistré", body = openapi::CoffreAlerts),
        (status = 401, description = "Pas connecté", body = ErrorBody),
        (status = 403, description = "Réservé à « Gérer le serveur »", body = ErrorBody),
        (status = 422, description = "Saisie refusée", body = ErrorBody),
    )
)]
pub async fn alerts_put(
    State(ctx): State<Context>,
    session: Session,
    Path(guild): Path<String>,
    JsonBody(body): JsonBody<openapi::CoffreAlerts>,
) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    let gid = guild_id.to_string();
    let wanted: alerts::CoffreAlerts = convert(&body).map_err(|_| ApiError::invalid("Réglage illisible."))?;
    wanted.validate().map_err(ApiError::invalid)?;
    alerts::set_alerts(&ctx.db, &gid, &wanted).await?;
    record_config(&ctx, &session, guild_id, "coffre", "alertes modifiées depuis le site".to_owned()).await;
    crate::modules::coffres::dashboard::request_refresh(&ctx, &gid);
    let saved = alerts::get_alerts(&ctx.db, &gid).await?;
    Ok(json(&convert::<_, openapi::CoffreAlerts>(&saved)?))
}

/// État de la configuration des modules (« Gérer le serveur »).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/config", tag = "reglages", operation_id = "getConfigStatus",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "État des modules", body = openapi::ConfigStatus),
        (status = 401, description = "Pas connecté", body = ErrorBody),
        (status = 403, description = "Réservé à « Gérer le serveur »", body = ErrorBody),
    )
)]
pub async fn config_get(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.require_admin(&guild)?;
    let status = crate::modules::status::config_status(&ctx, guild_id).await?;
    Ok(json(&convert::<_, openapi::ConfigStatus>(&status)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les structures des modules et celles du site doivent garder la même forme JSON.
    #[test]
    fn module_and_api_shapes_match() {
        let goal = objective::Objective { amount: 50_000, counts: objective::Counts::Sale, reminder: true };
        let api: openapi::Objective = convert(&goal).unwrap();
        assert_eq!((api.amount, api.counts.as_str(), api.reminder), (50_000, "sale", true));
        assert_eq!(convert::<_, objective::Objective>(&api).unwrap(), goal);

        let raw = serde_json::json!({
            "channelId": "1", "roleId": null, "bigWithdrawal": 20, "unknownPlayer": true,
            "thresholds": [{ "propertyId": null, "item": "repair_kit", "min": 5 }],
        });
        let module: alerts::CoffreAlerts = serde_json::from_value(raw.clone()).unwrap();
        let api: openapi::CoffreAlerts = convert(&module).unwrap();
        assert_eq!(serde_json::to_value(&api).unwrap(), raw);
        assert_eq!(serde_json::to_value(convert::<_, alerts::CoffreAlerts>(&api).unwrap()).unwrap(), raw);
    }
}
