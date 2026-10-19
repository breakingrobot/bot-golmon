//! Objectif hebdomadaire de cotisation (lundi 00:00 → dimanche 23:59, heure de
//! Paris). Membres suivis : porteurs du rôle « membre » des permissions, sinon
//! du rôle des présences. Sont exemptés pour la semaine les membres ayant une
//! absence déclarée qui la chevauche et les recrues de la semaine.

use super::{
    money,
    service::{self, Filter, Totals},
};
use crate::{
    context::Ctx,
    discord::{members::role_member_ids, messages::parse_id},
    util::time::{DbTime, TIMEZONE, paris_day_start, week_start},
};
use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use twilight_model::id::{Id, marker::GuildMarker};

/// Objectif maximal (comme 10 cotisations au montant maximal).
pub const MAX_OBJECTIVE: i64 = 1_000_000_000;

/// Ce qui compte pour l'objectif.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Counts {
    #[default]
    Total,
    Propre,
    Sale,
}

impl Counts {
    pub const ALL: [Self; 3] = [Self::Total, Self::Propre, Self::Sale];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Total => "total",
            Self::Propre => "propre",
            Self::Sale => "sale",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == raw.trim())
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Total => "propre + sale",
            Self::Propre => "argent propre",
            Self::Sale => "argent sale",
        }
    }

    /// Montant qui compte dans des totaux.
    pub fn amount(self, totals: &Totals) -> i64 {
        match self {
            Self::Total => totals.total(),
            Self::Propre => totals.propre,
            Self::Sale => totals.sale,
        }
    }
}

/// Objectif hebdomadaire, tel qu'échangé avec le site (`Objective` côté TypeScript).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub amount: i64,
    pub counts: Counts,
    /// Rappel le dimanche aux membres en dessous de l'objectif.
    pub reminder: bool,
}

impl Objective {
    /// « 50 000 $ (propre + sale) ».
    pub fn describe(&self) -> String {
        format!("{} ({})", money(self.amount), self.counts.label())
    }
}

/// Avancement d'un membre sur la semaine (identifiants, pas de noms).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressRow {
    pub user_id: String,
    pub amount: i64,
    pub target: i64,
    pub reached: bool,
    /// Absence déclarée ou recrue : non concerné cette semaine.
    pub exempt: bool,
}

/// Objectif du serveur, `None` s'il n'y en a pas.
pub async fn get_objective(db: &SqlitePool, guild_id: &str) -> Result<Option<Objective>> {
    let row: Option<(Option<i64>, String, bool)> = sqlx::query_as(
        r#"SELECT "objectiveAmount", "objectiveCounts", "objectiveReminder" FROM "CotisationConfig" WHERE "guildId" = ?"#,
    )
    .bind(guild_id)
    .fetch_optional(db)
    .await?;
    Ok(row.and_then(|(amount, counts, reminder)| {
        let amount = amount.filter(|a| *a > 0)?;
        Some(Objective { amount, counts: Counts::parse(&counts).unwrap_or_default(), reminder })
    }))
}

/// Règle l'objectif ; un montant de 0 le désactive. Un objectif qui démarre
/// ne déclenche pas le bilan d'une semaine passée sans objectif.
pub async fn set_objective(db: &SqlitePool, guild_id: &str, objective: &Objective) -> Result<()> {
    if !(0..=MAX_OBJECTIVE).contains(&objective.amount) {
        bail!("L'objectif doit être entre 0 et {}.", money(MAX_OBJECTIVE));
    }
    let was_active = get_objective(db, guild_id).await?.is_some();
    let now = DbTime::now();
    let amount = (objective.amount > 0).then_some(objective.amount);
    sqlx::query(
        r#"INSERT INTO "CotisationConfig" ("guildId", "objectiveAmount", "objectiveCounts", "objectiveReminder", "lastRecapAt", "updatedAt")
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT ("guildId") DO UPDATE SET "objectiveAmount" = excluded."objectiveAmount",
             "objectiveCounts" = excluded."objectiveCounts", "objectiveReminder" = excluded."objectiveReminder",
             "lastRecapAt" = CASE WHEN ?7 THEN "lastRecapAt" ELSE excluded."lastRecapAt" END,
             "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(amount)
    .bind(objective.counts.as_str())
    .bind(objective.reminder)
    .bind(now)
    .bind(now)
    .bind(was_active)
    .execute(db)
    .await?;
    Ok(())
}

/// Lundi 00:00 (heure de Paris) suivant `week_start`, exclu de la semaine.
pub fn week_end(week_start: DbTime) -> DbTime {
    paris_day_start(monday_of(week_start) + Duration::days(7))
}

/// Lundi (jour de Paris) d'une semaine.
pub fn monday_of(week: DbTime) -> NaiveDate {
    week_start(week.0).0.with_timezone(&TIMEZONE).date_naive()
}

/// Semaine précédente.
pub fn previous_week(week: DbTime) -> DbTime {
    week_start(week_start(week.0).0 - Duration::days(1))
}

/// « du 13/10 au 19/10 ».
pub fn week_label(week: DbTime) -> String {
    let monday = monday_of(week);
    format!("du {} au {}", monday.format("%d/%m"), (monday + Duration::days(6)).format("%d/%m"))
}

/// Membres exemptés de l'objectif cette semaine : absence déclarée qui
/// chevauche la semaine (une absence annulée avant son début ne compte pas),
/// ou recrue de la semaine.
pub async fn exempt_users(db: &SqlitePool, guild_id: &str, week: DbTime) -> Result<HashSet<String>> {
    let start = week_start(week.0);
    let end = week_end(start);
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "userId" FROM "Absence"
             WHERE "guildId" = ?1 AND "startsAt" < ?3 AND COALESCE("endedAt", "endsAt") > ?2
               AND COALESCE("endedAt", "endsAt") > "startsAt"
           UNION
           SELECT e."userId" FROM "RecrueEntry" e JOIN "RecrueSession" s ON s."id" = e."sessionId"
             WHERE s."guildId" = ?1 AND s."weekStart" = ?2"#,
    )
    .bind(guild_id)
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Totaux de chaque membre sur la semaine.
pub async fn week_totals(db: &SqlitePool, guild_id: &str, week: DbTime) -> Result<HashMap<String, Totals>> {
    let start = week_start(week.0);
    let filter = Filter { since: Some(start), until: Some(week_end(start)), ..Default::default() };
    Ok(service::by_member(db, guild_id, &filter).await?.into_iter().collect())
}

/// Avancement des membres suivis, du plus loin de l'objectif au plus proche ;
/// ceux qui l'ont atteint ensuite, les exemptés à la fin.
pub fn compute_progress(
    objective: &Objective,
    tracked: &[String],
    totals: &HashMap<String, Totals>,
    exempt: &HashSet<String>,
) -> Vec<ProgressRow> {
    let mut rows: Vec<ProgressRow> = tracked
        .iter()
        .map(|user_id| {
            let amount = totals.get(user_id).map_or(0, |t| objective.counts.amount(t));
            ProgressRow {
                user_id: user_id.clone(),
                amount,
                target: objective.amount,
                reached: amount >= objective.amount,
                exempt: exempt.contains(user_id),
            }
        })
        .collect();
    rows.sort_by(|a, b| (a.exempt, a.reached, a.amount, &a.user_id).cmp(&(b.exempt, b.reached, b.amount, &b.user_id)));
    rows
}

/// Avancement de la semaine pour une liste de membres.
pub async fn progress_for(
    db: &SqlitePool,
    guild_id: &str,
    week: DbTime,
    objective: &Objective,
    tracked: &[String],
) -> Result<Vec<ProgressRow>> {
    let totals = week_totals(db, guild_id, week).await?;
    let exempt = exempt_users(db, guild_id, week).await?;
    Ok(compute_progress(objective, tracked, &totals, &exempt))
}

/// Avancement d'un membre (suivi ou non) sur la semaine.
pub async fn member_progress(
    db: &SqlitePool,
    guild_id: &str,
    user_id: &str,
    week: DbTime,
    objective: &Objective,
) -> Result<ProgressRow> {
    let rows = progress_for(db, guild_id, week, objective, &[user_id.to_owned()]).await?;
    Ok(rows.into_iter().next().expect("une ligne par membre demandé"))
}

/// Rôle des membres suivis : rôle « membre » des permissions, sinon rôle des présences.
pub async fn tracked_role(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    if let Some(role) = crate::modules::permissions::service::member_role(db, guild_id).await? {
        return Ok(Some(role));
    }
    Ok(crate::modules::presences::service::get_config(db, guild_id).await?.and_then(|c| c.default_role_id))
}

/// Membres suivis (cache des membres), hors bots.
pub async fn tracked_members(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<Vec<String>> {
    let Some(role) = tracked_role(&ctx.db, &guild_id.to_string()).await?.as_deref().and_then(parse_id) else {
        return Ok(Vec::new());
    };
    Ok(role_member_ids(ctx, guild_id, role)
        .into_iter()
        .filter(|id| !parse_id(id).and_then(|id| ctx.cache.user(id)).is_some_and(|user| user.bot))
        .collect())
}

/// Avancement de chaque membre suivi sur la semaine de `week_start` (vide sans objectif).
pub async fn objective_progress(ctx: &Ctx, guild_id: Id<GuildMarker>, week_start: DbTime) -> Result<Vec<ProgressRow>> {
    let guild = guild_id.to_string();
    let Some(objective) = get_objective(&ctx.db, &guild).await? else {
        return Ok(Vec::new());
    };
    let tracked = tracked_members(ctx, guild_id).await?;
    progress_for(&ctx.db, &guild, week_start, &objective, &tracked).await
}

/// Barre d'avancement textuelle : « ▰▰▰▱▱▱▱▱▱▱ 30 % ».
pub fn progress_bar(amount: i64, target: i64) -> String {
    let percent = if target > 0 { (amount.max(0) as f64 / target as f64 * 100.0).round() as i64 } else { 100 };
    let filled = (percent.clamp(0, 100) / 10) as usize;
    format!("{}{} {percent} %", "▰".repeat(filled), "▱".repeat(10 - filled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::cotisations::service::{NewCotisation, add};
    use chrono::{TimeZone, Utc};

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn week_bounds_follow_paris_time() {
        // Semaine du passage à l'heure d'hiver (dimanche 25/10/2026).
        let monday = week_start(Utc.with_ymd_and_hms(2026, 10, 21, 12, 0, 0).unwrap());
        assert_eq!(monday.to_db_string(), "2026-10-18T22:00:00.000Z");
        assert_eq!(week_end(monday).to_db_string(), "2026-10-25T23:00:00.000Z");
        assert_eq!(previous_week(monday).to_db_string(), "2026-10-11T22:00:00.000Z");
        assert_eq!(week_label(monday), "du 19/10 au 25/10");
        // Une date quelconque de la semaine donne les mêmes bornes.
        assert_eq!(week_end(DbTime(Utc.with_ymd_and_hms(2026, 10, 25, 21, 0, 0).unwrap())), week_end(monday));
    }

    #[test]
    fn progress_sorted_and_flagged() {
        let objective = Objective { amount: 1000, counts: Counts::Propre, reminder: true };
        let totals: HashMap<String, Totals> = [
            ("a".to_owned(), Totals { count: 1, propre: 1000, sale: 0 }),
            ("b".to_owned(), Totals { count: 2, propre: 200, sale: 5000 }),
            ("c".to_owned(), Totals { count: 1, propre: 0, sale: 900 }),
        ]
        .into();
        let exempt: HashSet<String> = ["d".to_owned()].into();
        let rows = compute_progress(&objective, &ids(&["a", "b", "c", "d"]), &totals, &exempt);
        let summary: Vec<_> = rows.iter().map(|r| (r.user_id.as_str(), r.amount, r.reached, r.exempt)).collect();
        assert_eq!(
            summary,
            [("c", 0, false, false), ("b", 200, false, false), ("a", 1000, true, false), ("d", 0, false, true)]
        );
        let total = Objective { counts: Counts::Total, ..objective };
        assert!(compute_progress(&total, &ids(&["b"]), &totals, &exempt)[0].reached);
        assert_eq!(progress_bar(250, 1000), "▰▰▱▱▱▱▱▱▱▱ 25 %");
        assert_eq!(progress_bar(5000, 1000), "▰▰▰▰▰▰▰▰▰▰ 500 %");
    }

    #[test]
    fn serializes_like_the_typescript_contract() {
        let objective = Objective { amount: 50_000, counts: Counts::Sale, reminder: false };
        let json = serde_json::to_value(objective).unwrap();
        assert_eq!(json, serde_json::json!({ "amount": 50000, "counts": "sale", "reminder": false }));
        assert_eq!(serde_json::from_value::<Objective>(json).unwrap(), objective);
        let row = ProgressRow { user_id: "1".into(), amount: 2, target: 3, reached: false, exempt: true };
        assert_eq!(
            serde_json::to_value(row).unwrap(),
            serde_json::json!({ "userId": "1", "amount": 2, "target": 3, "reached": false, "exempt": true })
        );
    }

    #[tokio::test]
    async fn progress_with_exemptions_and_week_boundaries() {
        let dir = std::env::temp_dir().join(format!("objectif-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        assert_eq!(get_objective(&db, "g").await.unwrap(), None);
        let objective = Objective { amount: 1000, counts: Counts::Total, reminder: true };
        set_objective(&db, "g", &objective).await.unwrap();
        assert_eq!(get_objective(&db, "g").await.unwrap(), Some(objective));
        assert!(set_objective(&db, "g", &Objective { amount: -1, ..objective }).await.is_err());

        // Semaine du lundi 12/10/2026 (Paris, UTC+2).
        let week = week_start(Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap());
        let paris = |d: u32, h: u32| DbTime(TIMEZONE.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap().to_utc());
        for (user, propre, sale, date) in [
            ("a", 600, 0, paris(12, 0)),  // lundi minuit : compte
            ("a", 0, 500, paris(18, 0)),  // dimanche : compte
            ("b", 5000, 0, paris(19, 0)), // lundi suivant : ne compte pas
            ("b", 300, 0, paris(11, 0)),  // dimanche précédent : ne compte pas
            ("c", 100, 0, paris(15, 0)),
        ] {
            let new = NewCotisation {
                user_id: user.into(),
                propre,
                sale,
                description: "test".into(),
                date,
                created_by_id: user.into(),
            };
            add(&db, "g", new).await.unwrap();
        }
        // « d » est absent en milieu de semaine, « e » est recrue cette semaine,
        // « f » a annulé son absence avant qu'elle commence, « h » est revenu avant la semaine.
        for (id, user, starts, ends, ended) in [
            ("ab1", "d", paris(14, 0), paris(16, 0), None),
            ("ab2", "f", paris(14, 0), paris(16, 0), Some(paris(10, 0))),
            ("ab3", "h", paris(1, 0), paris(20, 0), Some(paris(11, 12))),
        ] {
            sqlx::query(
                r#"INSERT INTO "Absence" ("id", "guildId", "userId", "motif", "startsAt", "endsAt", "createdById", "endedAt")
                   VALUES (?, 'g', ?, 'test', ?, ?, ?, ?)"#,
            )
            .bind(id)
            .bind(user)
            .bind(starts)
            .bind(ends)
            .bind(user)
            .bind(ended)
            .execute(&db)
            .await
            .unwrap();
        }
        sqlx::query(
            r#"INSERT INTO "RecrueSession" ("id", "guildId", "weekStart", "createdAt") VALUES ('s1', 'g', ?, ?)"#,
        )
        .bind(week)
        .bind(week)
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO "RecrueEntry" ("id", "sessionId", "guildId", "userId", "createdAt") VALUES ('e1', 's1', 'g', 'e', ?)"#,
        )
        .bind(week)
        .execute(&db)
        .await
        .unwrap();

        let exempt = exempt_users(&db, "g", week).await.unwrap();
        assert_eq!(exempt, ["d".to_owned(), "e".to_owned()].into());
        // La semaine précédente, seul « h » (rentré le dimanche 11 à midi) est exempté.
        assert_eq!(exempt_users(&db, "g", previous_week(week)).await.unwrap(), ["h".to_owned()].into());

        let rows = progress_for(&db, "g", week, &objective, &ids(&["a", "b", "c", "d", "e"])).await.unwrap();
        let summary: Vec<_> = rows.iter().map(|r| (r.user_id.as_str(), r.amount, r.reached, r.exempt)).collect();
        assert_eq!(
            summary,
            [
                ("b", 0, false, false),
                ("c", 100, false, false),
                ("a", 1100, true, false),
                ("d", 0, false, true),
                ("e", 0, false, true)
            ]
        );
        let solo = member_progress(&db, "g", "b", previous_week(week), &objective).await.unwrap();
        assert_eq!(solo.amount, 300);

        // Désactivé avec un montant de 0.
        set_objective(&db, "g", &Objective { amount: 0, ..objective }).await.unwrap();
        assert_eq!(get_objective(&db, "g").await.unwrap(), None);

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
