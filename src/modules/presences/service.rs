use super::{
    blocklist,
    embed::{build_buttons, build_embed},
    model::{Presence, PresenceConfig, PresenceResponse, ResponseChange, ResponseStatus},
};
use crate::{
    context::Ctx,
    discord::{
        members::{guild_icon_url, role_member_ids},
        messages::{self, Mentions, Outgoing, parse_id},
    },
    modules::like_contains,
    util::{format::chunk_mentions, ids::new_id, time::DbTime},
};
use anyhow::Result;
use chrono::Duration;
use sqlx::SqlitePool;
use std::collections::HashSet;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, GuildMarker},
};

const PRESENCE_COLUMNS: &str = r#""id", "guildId", "channelId", "messageId", "titre", "description", "eventDate", "roleId", "createdById", "createdAt", "closedAt""#;

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Option<PresenceConfig>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "PresenceConfig" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_optional(db)
        .await?)
}

/// Les champs absents (`None`) conservent leur valeur actuelle.
#[derive(Default)]
pub struct ConfigUpdate {
    pub default_role_id: Option<String>,
    pub absence_role_id: Option<String>,
    pub presence_channel_id: Option<String>,
    pub reminder_channel_id: Option<String>,
    pub absence_channel_id: Option<String>,
    pub report_channel_id: Option<String>,
    pub alert_threshold: Option<i64>,
    /// Arrête le rapport hebdomadaire (efface le salon du rapport).
    pub disable_report: bool,
}

impl ConfigUpdate {
    pub fn is_empty(&self) -> bool {
        self.default_role_id.is_none()
            && self.absence_role_id.is_none()
            && self.presence_channel_id.is_none()
            && self.reminder_channel_id.is_none()
            && self.absence_channel_id.is_none()
            && self.report_channel_id.is_none()
            && self.alert_threshold.is_none()
            && !self.disable_report
    }
}

pub async fn upsert_config(db: &SqlitePool, guild_id: &str, update: ConfigUpdate) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "PresenceConfig" ("guildId", "defaultRoleId", "absenceRoleId", "presenceChannelId", "reminderChannelId", "absenceChannelId", "reportChannelId", "alertThreshold", "updatedAt")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET
             "defaultRoleId" = COALESCE(excluded."defaultRoleId", "defaultRoleId"),
             "absenceRoleId" = COALESCE(excluded."absenceRoleId", "absenceRoleId"),
             "presenceChannelId" = COALESCE(excluded."presenceChannelId", "presenceChannelId"),
             "reminderChannelId" = COALESCE(excluded."reminderChannelId", "reminderChannelId"),
             "absenceChannelId" = COALESCE(excluded."absenceChannelId", "absenceChannelId"),
             "reportChannelId" = CASE WHEN ? THEN NULL ELSE COALESCE(excluded."reportChannelId", "reportChannelId") END,
             "alertThreshold" = COALESCE(excluded."alertThreshold", "alertThreshold"),
             "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(update.default_role_id)
    .bind(update.absence_role_id)
    .bind(update.presence_channel_id)
    .bind(update.reminder_channel_id)
    .bind(update.absence_channel_id)
    .bind(update.report_channel_id)
    .bind(update.alert_threshold)
    .bind(DbTime::now())
    .bind(update.disable_report)
    .execute(db)
    .await?;
    Ok(())
}

pub struct NewPresence {
    pub channel_id: String,
    pub titre: String,
    pub description: Option<String>,
    pub event_date: DbTime,
    pub role_id: String,
    pub absence_role_id: Option<String>,
    pub created_by_id: String,
}

pub async fn create_presence(ctx: &Ctx, guild_id: Id<GuildMarker>, params: NewPresence) -> Result<Presence> {
    let id = new_id();
    let now = DbTime::now();
    let auto_absent = auto_absent_user_ids(ctx, guild_id, params.event_date, params.absence_role_id.as_deref()).await?;

    let mut tx = ctx.db.begin().await?;
    sqlx::query(
        r#"INSERT INTO "Presence" ("id", "guildId", "channelId", "titre", "description", "eventDate", "roleId", "absenceRoleId", "createdById", "createdAt", "lastReminderAt")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(guild_id.to_string())
    .bind(&params.channel_id)
    .bind(&params.titre)
    .bind(&params.description)
    .bind(params.event_date)
    .bind(&params.role_id)
    .bind(&params.absence_role_id)
    .bind(&params.created_by_id)
    .bind(now)
    // La création pingue déjà tout le monde : le premier rappel tombera 3 h après.
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // Absences automatiques : pas d'historique de changement, ce ne sont pas
    // des actions des membres.
    for user_id in &auto_absent {
        sqlx::query(
            r#"INSERT INTO "PresenceResponse" ("id", "presenceId", "userId", "status", "respondedAt", "updatedAt") VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(new_id())
        .bind(&id)
        .bind(user_id)
        .bind(ResponseStatus::Absent.as_str())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    get_presence(&ctx.db, &id).await?.ok_or_else(|| anyhow::anyhow!("présence {id} introuvable après création"))
}

/// Crée la présence, publie son message et pingue les membres du rôle (hors
/// absents et blocklist). `None` si Discord refuse la publication : la présence
/// est alors retirée de la base plutôt que de rester invisible.
pub async fn publish_presence(ctx: &Ctx, guild_id: Id<GuildMarker>, params: NewPresence) -> Result<Option<Presence>> {
    let channel_id = parse_id::<ChannelMarker>(&params.channel_id);
    let role_id = params.role_id.clone();
    let absence_role_id = params.absence_role_id.clone();
    let event_date = params.event_date;
    let presence = create_presence(ctx, guild_id, params).await?;

    if let Err(error) = sync_message(ctx, &presence.id).await {
        tracing::error!(presence = %presence.id, ?error, "publication de la présence échouée, suppression");
        let _ = delete_presence(&ctx.db, &presence.id).await;
        return Ok(None);
    }

    if let Some(channel_id) = channel_id {
        let targets = parse_id(&role_id).map(|role| role_member_ids(ctx, guild_id, role)).unwrap_or_default();
        let excluded: HashSet<String> =
            auto_absent_user_ids(ctx, guild_id, event_date, absence_role_id.as_deref()).await?.into_iter().collect();
        let mentionable: Vec<String> = targets.into_iter().filter(|id| !excluded.contains(id)).collect();
        for content in chunk_mentions(&mentionable, "📣") {
            messages::send(ctx, channel_id, &Outgoing::text(content).with_mentions(Mentions::Users)).await?;
        }
    }
    Ok(Some(presence))
}

/// Membres marqués absents d'office et jamais pingés : porteurs du rôle
/// d'absence, membres de la blocklist et absences déclarées couvrant la date.
pub async fn auto_absent_user_ids(
    ctx: &Ctx,
    guild_id: Id<GuildMarker>,
    event_date: DbTime,
    absence_role_id: Option<&str>,
) -> Result<Vec<String>> {
    let mut ids: Vec<String> =
        absence_role_id.and_then(parse_id).map(|role_id| role_member_ids(ctx, guild_id, role_id)).unwrap_or_default();
    ids.extend(blocklist::user_ids(&ctx.db, &guild_id.to_string()).await?);

    let covering: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "userId" FROM "Absence" WHERE "guildId" = ? AND "endedAt" IS NULL AND "startsAt" <= ? AND "endsAt" >= ?"#,
    )
    .bind(guild_id.to_string())
    .bind(event_date)
    .bind(event_date)
    .fetch_all(&ctx.db)
    .await?;
    ids.extend(covering.into_iter().map(|(id,)| id));

    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    Ok(ids)
}

/// Enregistre la réponse d'un membre. Renvoie `false` si rien n'a changé, pour
/// éviter de rééditer le message pour rien.
pub async fn register_response(
    db: &SqlitePool,
    presence_id: &str,
    user_id: &str,
    status: ResponseStatus,
    retard_minutes: Option<i64>,
) -> Result<bool> {
    let mut tx = db.begin().await?;
    let existing: Option<(String, String, Option<i64>)> = sqlx::query_as(
        r#"SELECT "id", "status", "retardMinutes" FROM "PresenceResponse" WHERE "presenceId" = ? AND "userId" = ?"#,
    )
    .bind(presence_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some((_, current, minutes)) = &existing
        && current == status.as_str()
        && *minutes == retard_minutes
    {
        return Ok(false);
    }

    let now = DbTime::now();
    let response_id = match &existing {
        Some((id, ..)) => {
            sqlx::query(r#"UPDATE "PresenceResponse" SET "status" = ?, "retardMinutes" = ?, "respondedAt" = ?, "updatedAt" = ? WHERE "id" = ?"#)
                .bind(status.as_str())
                .bind(retard_minutes)
                .bind(now)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            id.clone()
        }
        None => {
            let id = new_id();
            sqlx::query(
                r#"INSERT INTO "PresenceResponse" ("id", "presenceId", "userId", "status", "retardMinutes", "respondedAt", "updatedAt") VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(&id)
            .bind(presence_id)
            .bind(user_id)
            .bind(status.as_str())
            .bind(retard_minutes)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            id
        }
    };

    sqlx::query(r#"INSERT INTO "PresenceResponseChange" ("id", "responseId", "fromStatus", "toStatus", "changedAt") VALUES (?, ?, ?, ?, ?)"#)
        .bind(new_id())
        .bind(&response_id)
        .bind(existing.map(|(_, status, _)| status))
        .bind(status.as_str())
        .bind(now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn get_presence(db: &SqlitePool, presence_id: &str) -> Result<Option<Presence>> {
    Ok(sqlx::query_as(&format!(r#"SELECT {PRESENCE_COLUMNS} FROM "Presence" WHERE "id" = ?"#))
        .bind(presence_id)
        .fetch_optional(db)
        .await?)
}

/// Présence de la guilde : l'identifiant figure dans le pied de l'embed et
/// l'autocomplétion n'empêche pas de saisir une valeur arbitraire.
pub async fn get_guild_presence(db: &SqlitePool, guild_id: &str, presence_id: &str) -> Result<Option<Presence>> {
    Ok(sqlx::query_as(&format!(r#"SELECT {PRESENCE_COLUMNS} FROM "Presence" WHERE "id" = ? AND "guildId" = ?"#))
        .bind(presence_id)
        .bind(guild_id)
        .fetch_optional(db)
        .await?)
}

pub async fn responses(db: &SqlitePool, presence_id: &str) -> Result<Vec<PresenceResponse>> {
    Ok(sqlx::query_as(
        r#"SELECT r."userId", r."status", r."retardMinutes",
                  EXISTS (SELECT 1 FROM "PresenceResponseChange" c WHERE c."responseId" = r."id" AND c."fromStatus" IS NOT NULL) AS "changed"
           FROM "PresenceResponse" r WHERE r."presenceId" = ? ORDER BY r."respondedAt" ASC, r."id" ASC"#,
    )
    .bind(presence_id)
    .fetch_all(db)
    .await?)
}

/// Changements d'avis (hors première réponse) d'une présence, plus récents d'abord.
pub async fn presence_changes(db: &SqlitePool, presence_id: &str) -> Result<Vec<ResponseChange>> {
    Ok(sqlx::query_as(
        r#"SELECT r."userId", c."fromStatus", c."toStatus", c."changedAt", p."titre"
           FROM "PresenceResponseChange" c
           JOIN "PresenceResponse" r ON r."id" = c."responseId"
           JOIN "Presence" p ON p."id" = r."presenceId"
           WHERE r."presenceId" = ? AND c."fromStatus" IS NOT NULL
           ORDER BY r."userId", c."changedAt" DESC"#,
    )
    .bind(presence_id)
    .fetch_all(db)
    .await?)
}

pub struct Roster {
    pub effectif_total: usize,
    pub non_respondent_ids: Vec<String>,
}

/// Effectif du rôle ciblé et membres n'ayant pas encore répondu. Les absents
/// d'office sont déjà enregistrés en ABSENT à la création, donc exclus.
pub fn compute_roster(ctx: &Ctx, guild_id: Id<GuildMarker>, role_id: &str, responded: &HashSet<&str>) -> Roster {
    let members = parse_id(role_id).map(|role| role_member_ids(ctx, guild_id, role)).unwrap_or_default();
    Roster {
        effectif_total: members.len(),
        non_respondent_ids: members.into_iter().filter(|id| !responded.contains(id.as_str())).collect(),
    }
}

pub async fn non_respondent_ids(ctx: &Ctx, presence: &Presence) -> Result<Vec<String>> {
    let Some(guild_id) = parse_id(&presence.guild_id) else {
        return Ok(Vec::new());
    };
    let responses = responses(&ctx.db, &presence.id).await?;
    let responded: HashSet<&str> = responses.iter().map(|r| r.user_id.as_str()).collect();
    Ok(compute_roster(ctx, guild_id, &presence.role_id, &responded).non_respondent_ids)
}

pub async fn close_presence(db: &SqlitePool, presence_id: &str) -> Result<()> {
    sqlx::query(r#"UPDATE "Presence" SET "closedAt" = ? WHERE "id" = ?"#)
        .bind(DbTime::now())
        .bind(presence_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_presence(db: &SqlitePool, presence_id: &str) -> Result<()> {
    sqlx::query(r#"DELETE FROM "Presence" WHERE "id" = ?"#).bind(presence_id).execute(db).await?;
    Ok(())
}

/// Délai de grâce avant de considérer qu'une présence sans message a échoué.
const ORPHAN_GRACE_MINUTES: i64 = 5;

/// Vérifie qu'une présence est encore exploitable, c'est-à-dire que son message
/// Discord existe toujours. Sinon plus personne ne peut y répondre : on la
/// supprime au lieu de relancer les membres dans le vide.
///
/// Les statistiques ne comptent que les présences clôturées, et on ne touche
/// jamais à une présence déjà clôturée : aucun historique n'est perdu.
pub async fn ensure_alive(ctx: &Ctx, presence: &Presence) -> Result<bool> {
    let Some(message_id) = presence.message_id.as_deref().and_then(parse_id) else {
        if DbTime::now().0 - presence.created_at.0 < Duration::minutes(ORPHAN_GRACE_MINUTES) {
            // Création peut-être encore en cours : on laisse sa chance au prochain passage.
            return Ok(false);
        }
        delete_presence(&ctx.db, &presence.id).await?;
        tracing::warn!(presence = %presence.id, "présence supprimée : message jamais publié");
        return Ok(false);
    };

    if let Some(channel_id) = parse_id(&presence.channel_id)
        && messages::fetch(ctx, channel_id, message_id).await?.is_some()
    {
        return Ok(true);
    }

    delete_presence(&ctx.db, &presence.id).await?;
    tracing::warn!(presence = %presence.id, "présence supprimée : message introuvable");
    Ok(false)
}

pub async fn find_presences(
    db: &SqlitePool,
    guild_id: &str,
    query: &str,
    include_closed: bool,
) -> Result<Vec<Presence>> {
    let sql = if include_closed {
        format!(
            r#"SELECT {PRESENCE_COLUMNS} FROM "Presence" WHERE "guildId" = ? AND "titre" LIKE ? ESCAPE '\' ORDER BY "eventDate" DESC LIMIT 25"#
        )
    } else {
        format!(
            r#"SELECT {PRESENCE_COLUMNS} FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NULL AND "titre" LIKE ? ESCAPE '\' ORDER BY "eventDate" ASC LIMIT 25"#
        )
    };
    Ok(sqlx::query_as(&sql).bind(guild_id).bind(like_contains(query)).fetch_all(db).await?)
}

/// Titres des dernières présences, sans doublon, pour l'autocomplétion.
pub async fn recent_titles(db: &SqlitePool, guild_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "titre" FROM "Presence" WHERE "guildId" = ? GROUP BY "titre" ORDER BY MAX("eventDate") DESC LIMIT 50"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(titre,)| titre).collect())
}

/// Présence ouverte la plus proche, utilisée par défaut quand aucune n'est précisée.
pub async fn current_presence(db: &SqlitePool, guild_id: &str) -> Result<Option<Presence>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {PRESENCE_COLUMNS} FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NULL ORDER BY "eventDate" ASC LIMIT 1"#
    ))
    .bind(guild_id)
    .fetch_optional(db)
    .await?)
}

pub async fn reset_guild(db: &SqlitePool, guild_id: &str) -> Result<()> {
    sqlx::query(r#"DELETE FROM "Presence" WHERE "guildId" = ?"#).bind(guild_id).execute(db).await?;
    Ok(())
}

pub struct UserStats {
    pub total_closed: i64,
    pub present: i64,
    pub absent: i64,
    pub retard: i64,
    pub sans_reponse: i64,
    pub percent: i64,
    pub recent_changes: Vec<ResponseChange>,
}

fn percent(part: i64, total: i64) -> i64 {
    if total > 0 { ((part as f64 / total as f64) * 100.0).round() as i64 } else { 0 }
}

pub async fn user_stats(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<UserStats> {
    let (total_closed,): (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL"#)
            .bind(guild_id)
            .fetch_one(db)
            .await?;

    let counts: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT r."status", COUNT(*) FROM "PresenceResponse" r JOIN "Presence" p ON p."id" = r."presenceId"
           WHERE r."userId" = ? AND p."guildId" = ? AND p."closedAt" IS NOT NULL GROUP BY r."status""#,
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    let count = |status: ResponseStatus| counts.iter().find(|(s, _)| s == status.as_str()).map_or(0, |(_, n)| *n);
    let (present, absent, retard) =
        (count(ResponseStatus::Present), count(ResponseStatus::Absent), count(ResponseStatus::Retard));

    let recent_changes = sqlx::query_as(
        r#"SELECT r."userId", c."fromStatus", c."toStatus", c."changedAt", p."titre"
           FROM "PresenceResponseChange" c
           JOIN "PresenceResponse" r ON r."id" = c."responseId"
           JOIN "Presence" p ON p."id" = r."presenceId"
           WHERE r."userId" = ? AND p."guildId" = ? AND c."fromStatus" IS NOT NULL
           ORDER BY c."changedAt" DESC LIMIT 5"#,
    )
    .bind(user_id)
    .bind(guild_id)
    .fetch_all(db)
    .await?;

    Ok(UserStats {
        total_closed,
        present,
        absent,
        retard,
        sans_reponse: total_closed - (present + absent + retard),
        percent: percent(present, total_closed),
        recent_changes,
    })
}

pub struct LeaderboardEntry {
    pub user_id: String,
    pub present: i64,
    pub absent: i64,
    pub retard: i64,
    pub percent: i64,
}

pub async fn leaderboard(db: &SqlitePool, guild_id: &str) -> Result<Vec<LeaderboardEntry>> {
    let (total_closed,): (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "Presence" WHERE "guildId" = ? AND "closedAt" IS NOT NULL"#)
            .bind(guild_id)
            .fetch_one(db)
            .await?;

    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        r#"SELECT r."userId",
                  SUM(r."status" = 'PRESENT'), SUM(r."status" = 'ABSENT'), SUM(r."status" = 'RETARD')
           FROM "PresenceResponse" r JOIN "Presence" p ON p."id" = r."presenceId"
           WHERE p."guildId" = ? AND p."closedAt" IS NOT NULL
           GROUP BY r."userId""#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;

    let mut entries: Vec<LeaderboardEntry> = rows
        .into_iter()
        .map(|(user_id, present, absent, retard)| LeaderboardEntry {
            user_id,
            present,
            absent,
            retard,
            percent: percent(present, total_closed),
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.percent));
    Ok(entries)
}

/// Met à jour (ou publie) le message de la présence.
pub async fn sync_message(ctx: &Ctx, presence_id: &str) -> Result<()> {
    let Some(presence) = get_presence(&ctx.db, presence_id).await? else {
        return Ok(());
    };
    ctx.events.publish(&presence.guild_id, crate::events::Topic::Presences);
    let (Some(guild_id), Some(channel_id)) = (parse_id(&presence.guild_id), parse_id(&presence.channel_id)) else {
        return Ok(());
    };

    let responses = responses(&ctx.db, &presence.id).await?;
    let responded: HashSet<&str> = responses.iter().map(|r| r.user_id.as_str()).collect();
    let roster = compute_roster(ctx, guild_id, &presence.role_id, &responded);

    let embed = build_embed(&presence, &responses, &roster, guild_icon_url(ctx, guild_id));
    let count = |status| responses.iter().filter(|r| r.status() == status).count();
    let buttons = build_buttons(
        &presence.id,
        [count(ResponseStatus::Present), count(ResponseStatus::Absent), count(ResponseStatus::Retard)],
        presence.closed_at.is_some(),
    );
    let message = Outgoing::embeds(vec![embed]).with_components(vec![buttons]);

    if let Some(message_id) = presence.message_id.as_deref().and_then(parse_id) {
        if messages::fetch(ctx, channel_id, message_id).await?.is_some() {
            messages::edit(ctx, channel_id, message_id, &message).await?;
            return Ok(());
        }
        // Message supprimé : on ne republie pas. Une présence encore ouverte est
        // orpheline et supprimée ; une présence clôturée est conservée, elle ne
        // génère plus de rappels et ses statistiques comptent.
        if presence.closed_at.is_none() {
            delete_presence(&ctx.db, &presence.id).await?;
            tracing::warn!(presence = %presence.id, "présence supprimée : message introuvable");
        }
        return Ok(());
    }

    let sent = messages::send(ctx, channel_id, &message).await?;
    sqlx::query(r#"UPDATE "Presence" SET "messageId" = ? WHERE "id" = ?"#)
        .bind(sent.id.to_string())
        .bind(&presence.id)
        .execute(&ctx.db)
        .await?;
    Ok(())
}
