//! Recrues de la semaine : décompte des avis, vétos, avis détaillés et
//! historique (`recrue.suivi`), avis (`recrue.avis`) et vétos (`recrue.veto`)
//! depuis le site. Actions strictes : seuls les rôles comptent. Chaque écriture
//! met à jour le message d'avis de la semaine sur Discord, comme les menus.

use super::{ApiError, ApiResult, JsonBody, MemberRef, Members, Session, iso_db, json, stats::Veto};
use crate::{
    context::{Context, Ctx},
    modules::{
        audit::{self, Action, Entry as AuditEntry},
        permissions::Perm,
        recrues::{
            MOTIF_MAX_LENGTH, Vote,
            service::{self, Entry, Session as RecrueSession},
        },
    },
    util::time::{DbTime, week_start},
};
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};
use utoipa::ToSchema;

const ENTRY_GONE: &str = "Cette recrue n'est plus suivie (la semaine a peut-être été réinitialisée).";

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = RecrueVote)]
pub struct VoteDetail {
    pub voter: MemberRef,
    #[schema(value_type = super::openapi::RecrueChoice)]
    pub choice: &'static str,
    pub motif: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = RecrueMyVote)]
pub struct MyVote {
    #[schema(value_type = super::openapi::RecrueChoice)]
    pub choice: &'static str,
    pub motif: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recrue {
    /// Identifiant du suivi de la semaine (avis et vétos).
    pub entry_id: String,
    pub member: MemberRef,
    /// Première semaine suivie comme recrue.
    pub since: String,
    /// Semaines d'avis, celle-ci comprise.
    pub weeks: i64,
    pub pour: usize,
    pub contre: usize,
    pub blanc: usize,
    #[schema(required = true)]
    pub veto: Option<Veto>,
    /// Avis de la personne connectée, `null` si elle n'en a pas donné.
    #[schema(required = true)]
    pub my_vote: Option<MyVote>,
    /// Avis détaillés : `null` sans `recrue.suivi`.
    #[schema(required = true)]
    pub votes: Option<Vec<VoteDetail>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecruesResponse {
    /// Module configuré (rôles recrue et gradé).
    pub configured: bool,
    /// Lundi de la semaine en cours (jour de Paris), `null` sans semaine ouverte.
    #[schema(required = true)]
    pub week_start: Option<String>,
    pub recruits: Vec<Recrue>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecrueHistoryWeek {
    pub week_start: String,
    pub pour: usize,
    pub contre: usize,
    pub blanc: usize,
    #[schema(required = true)]
    pub veto: Option<Veto>,
    pub votes: Vec<VoteDetail>,
}

/// Page des recrues : avis, véto ou suivi (actions strictes).
pub fn can_view(session: &Session, guild_id: Id<GuildMarker>) -> bool {
    [Perm::RecrueSuivi, Perm::RecrueAvis, Perm::RecrueVeto].iter().any(|p| session.can(guild_id, *p))
}

fn choice(raw: &str) -> &'static str {
    Vote::parse(raw).map_or("BLANC", Vote::as_str)
}

fn vote_details(members: &Members, entry: &Entry) -> Vec<VoteDetail> {
    entry
        .votes
        .iter()
        .map(|v| VoteDetail { voter: members.get(&v.voter_id), choice: choice(&v.choix), motif: v.motif.clone() })
        .collect()
}

fn tally(entry: &Entry, vote: Vote) -> usize {
    entry.votes.iter().filter(|v| v.choix == vote.as_str()).count()
}

fn veto(members: &Members, entry: &Entry) -> Option<Veto> {
    entry
        .veto_by_id
        .as_ref()
        .map(|by| Veto { by: members.get(by), motif: entry.veto_motif.clone().unwrap_or_default() })
}

/// Arrivée dans le suivi et nombre de semaines, par membre.
async fn seniority(ctx: &Ctx, guild: &str) -> anyhow::Result<HashMap<String, (DbTime, i64)>> {
    let rows: Vec<(String, DbTime, i64)> = sqlx::query_as(
        r#"SELECT "userId", MIN("createdAt"), COUNT(*) FROM "RecrueEntry" WHERE "guildId" = ? GROUP BY "userId""#,
    )
    .bind(guild)
    .fetch_all(&ctx.db)
    .await?;
    Ok(rows.into_iter().map(|(user, since, weeks)| (user, (since, weeks))).collect())
}

fn is_current(session: &RecrueSession) -> bool {
    session.closed_at.is_none() && session.week_start == week_start(Utc::now())
}

/// Recrues de la semaine (`recrue.suivi`, `recrue.avis` ou `recrue.veto`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/recrues", tag = "recrues", operation_id = "getRecrues",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Recrues de la semaine", body = RecruesResponse),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn list(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    if !can_view(&session, guild_id) {
        return Err(session.denied(guild_id, Perm::RecrueAvis));
    }
    let gid = guild_id.to_string();
    let config = service::get_config(&ctx.db, &gid).await?;
    let configured = config.is_some_and(|c| c.recrue_role_id.is_some() && c.grade_role_id.is_some());
    let Some(week) = service::current_session(&ctx.db, &gid).await? else {
        return Ok(json(&RecruesResponse { configured, week_start: None, recruits: Vec::new() }));
    };
    let members = Members::new(&ctx, guild_id);
    let detailed = session.can(guild_id, Perm::RecrueSuivi);
    let seniority = seniority(&ctx, &gid).await?;
    let me = session.viewer.id.to_string();
    let recruits = week
        .entries
        .iter()
        .map(|entry| {
            let (since, weeks) = seniority.get(&entry.user_id).copied().unwrap_or((week.week_start, 1));
            Recrue {
                entry_id: entry.id.clone(),
                member: members.get(&entry.user_id),
                since: iso_db(since),
                weeks,
                pour: tally(entry, Vote::Pour),
                contre: tally(entry, Vote::Contre),
                blanc: tally(entry, Vote::Blanc),
                veto: veto(&members, entry),
                my_vote: entry
                    .votes
                    .iter()
                    .find(|v| v.voter_id == me)
                    .map(|v| MyVote { choice: choice(&v.choix), motif: v.motif.clone() }),
                votes: detailed.then(|| vote_details(&members, entry)),
            }
        })
        .collect();
    Ok(json(&RecruesResponse { configured, week_start: Some(super::paris_day(week.week_start)), recruits }))
}

/// Semaines d'avis d'un membre, de la plus récente à la plus ancienne (`recrue.suivi`).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/recrues/{id}/history", tag = "recrues", operation_id = "getRecrueHistory",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant Discord du membre")),
    responses(
        (status = 200, description = "Historique", body = Vec<RecrueHistoryWeek>),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
    )
)]
pub async fn history(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, user)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::RecrueSuivi)?;
    let user_id = super::parse_user(&user)?;
    let members = Members::new(&ctx, guild_id);
    let history = service::user_history(&ctx.db, &guild_id.to_string(), &user_id.to_string()).await?;
    let weeks: Vec<RecrueHistoryWeek> = history
        .iter()
        .map(|(entry, week)| RecrueHistoryWeek {
            week_start: super::paris_day(*week),
            pour: tally(entry, Vote::Pour),
            contre: tally(entry, Vote::Contre),
            blanc: tally(entry, Vote::Blanc),
            veto: veto(&members, entry),
            votes: vote_details(&members, entry),
        })
        .collect();
    Ok(json(&weeks))
}

/// Recrue de la semaine en cours, encore ouverte aux avis.
async fn current_entry(ctx: &Ctx, guild_id: Id<GuildMarker>, entry_id: &str) -> ApiResult<Entry> {
    let Some((entry, week)) = service::get_entry(&ctx.db, &guild_id.to_string(), entry_id.trim()).await? else {
        return Err(ApiError::not_found(ENTRY_GONE));
    };
    if !is_current(&week) {
        return Err(ApiError::invalid("Cette semaine d'avis est clôturée."));
    }
    Ok(entry)
}

/// Met à jour le message d'avis sans faire échouer l'action déjà enregistrée.
async fn refresh(ctx: &Ctx, guild_id: Id<GuildMarker>) {
    if let Err(error) = service::sync_dashboard(ctx, guild_id).await {
        tracing::warn!(?error, "message d'avis des recrues non mis à jour");
    }
}

fn check_motif(raw: &str) -> Result<String, String> {
    let motif = raw.trim().to_owned();
    if motif.is_empty() {
        return Err("Le motif est obligatoire.".to_owned());
    }
    if motif.chars().count() > usize::from(MOTIF_MAX_LENGTH) {
        return Err(format!("Le motif dépasse {MOTIF_MAX_LENGTH} caractères."));
    }
    Ok(motif)
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecrueVoteInput {
    #[schema(value_type = super::openapi::RecrueChoice)]
    pub choice: String,
    /// Obligatoire, 500 caractères au plus.
    pub motif: String,
}

/// Donne (ou remplace) son avis sur une recrue de la semaine (`recrue.avis`, stricte).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/recrues/{id}/vote", tag = "recrues", operation_id = "voteRecrue",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du suivi de la semaine")),
    request_body = RecrueVoteInput,
    responses(
        (status = 200, description = "Avis enregistré", body = MyVote),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn vote(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, entry_id)): Path<(String, String)>,
    JsonBody(input): JsonBody<RecrueVoteInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::RecrueAvis)?;
    let entry = current_entry(&ctx, guild_id, &entry_id).await?;
    let vote = Vote::parse(input.choice.trim()).ok_or_else(|| ApiError::invalid("Vote invalide."))?;
    let motif = check_motif(&input.motif).map_err(ApiError::invalid)?;
    service::cast_vote(&ctx.db, &entry.id, &session.viewer.id.to_string(), vote, &motif).await?;
    refresh(&ctx, guild_id).await;
    Ok(json(&MyVote { choice: vote.as_str(), motif }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecrueVetoInput {
    /// Obligatoire, 500 caractères au plus.
    pub motif: String,
}

async fn journal(ctx: &Ctx, session: &Session, guild_id: Id<GuildMarker>, entry: AuditEntry) {
    let actor = session.actor_name(ctx, guild_id);
    audit::record_web(ctx, guild_id, session.viewer.id, &actor, entry).await;
}

/// Pose un véto sur une recrue de la semaine (`recrue.veto`, stricte).
#[utoipa::path(
    put, path = "/api/guilds/{guild}/recrues/{id}/veto", tag = "recrues", operation_id = "setRecrueVeto",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du suivi de la semaine")),
    request_body = RecrueVetoInput,
    responses(
        (status = 200, description = "Véto posé", body = Veto),
        (status = 400, description = "Requête invalide", body = super::ErrorBody),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Saisie refusée", body = super::ErrorBody),
    )
)]
pub async fn set_veto(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, entry_id)): Path<(String, String)>,
    JsonBody(input): JsonBody<RecrueVetoInput>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::RecrueVeto)?;
    let entry = current_entry(&ctx, guild_id, &entry_id).await?;
    let members = Members::new(&ctx, guild_id);
    if let Some(by) = &entry.veto_by_id {
        return Err(ApiError::invalid(format!(
            "Cette recrue est déjà sous véto de {}. Lève-le d'abord.",
            members.get(by).name
        )));
    }
    let motif = check_motif(&input.motif).map_err(ApiError::invalid)?;
    service::set_veto(&ctx.db, &entry.id, &session.viewer.id.to_string(), &motif).await?;
    refresh(&ctx, guild_id).await;
    journal(&ctx, &session, guild_id, AuditEntry::new(Action::VetoSet, Some(entry.user_id.clone()), motif.clone()))
        .await;
    Ok(json(&Veto { by: members.get(&session.viewer.id.to_string()), motif }))
}

/// Lève le véto d'une recrue de la semaine (`recrue.veto`, stricte).
#[utoipa::path(
    delete, path = "/api/guilds/{guild}/recrues/{id}/veto", tag = "recrues", operation_id = "clearRecrueVeto",
    params(("guild" = String, Path, description = "Identifiant du serveur"), ("id" = String, Path, description = "Identifiant du suivi de la semaine")),
    responses(
        (status = 204, description = "Véto levé"),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Droit insuffisant", body = super::ErrorBody),
        (status = 404, description = "Introuvable", body = super::ErrorBody),
        (status = 422, description = "Aucun véto", body = super::ErrorBody),
    )
)]
pub async fn clear_veto(
    State(ctx): State<Context>,
    session: Session,
    Path((guild, entry_id)): Path<(String, String)>,
) -> ApiResult {
    let guild_id = session.require(&guild, Perm::RecrueVeto)?;
    let entry = current_entry(&ctx, guild_id, &entry_id).await?;
    if entry.veto_by_id.is_none() {
        return Err(ApiError::invalid("Aucun véto actif sur cette recrue."));
    }
    service::clear_veto(&ctx.db, &entry.id).await?;
    refresh(&ctx, guild_id).await;
    journal(&ctx, &session, guild_id, AuditEntry::new(Action::VetoLifted, Some(entry.user_id.clone()), String::new()))
        .await;
    Ok(super::no_content())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motifs_are_required_and_bounded() {
        assert_eq!(check_motif("  Sérieux "), Ok("Sérieux".to_owned()));
        assert_eq!(check_motif("   ").unwrap_err(), "Le motif est obligatoire.");
        assert!(check_motif(&"x".repeat(501)).is_err());
        assert_eq!(choice("CONTRE"), "CONTRE");
        assert_eq!(choice("??"), "BLANC");
    }
}
