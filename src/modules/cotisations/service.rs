//! Accès aux cotisations et statistiques (communes à Discord et au site).

use crate::util::{ids::new_id, time::DbTime};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Cotisation {
    pub id: String,
    pub user_id: String,
    pub propre: i64,
    pub sale: i64,
    pub description: String,
    pub date: DbTime,
    pub created_by_id: String,
}

impl Cotisation {
    pub fn total(&self) -> i64 {
        self.propre.saturating_add(self.sale)
    }
}

pub struct NewCotisation {
    pub user_id: String,
    pub propre: i64,
    pub sale: i64,
    pub description: String,
    pub date: DbTime,
    pub created_by_id: String,
}

/// Filtre commun : période (depuis `since`, inclus) et membre.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub since: Option<DbTime>,
    pub until: Option<DbTime>,
    pub user_id: Option<String>,
}

const COLUMNS: &str = r#""id", "userId", "propre", "sale", "description", "date", "createdById""#;
const WHERE: &str = r#""guildId" = ?1 AND (?2 IS NULL OR "date" >= ?2) AND (?3 IS NULL OR "date" < ?3) AND (?4 IS NULL OR "userId" = ?4)"#;

pub async fn add(db: &SqlitePool, guild_id: &str, new: NewCotisation) -> Result<Cotisation> {
    let id = new_id();
    let now = DbTime::now();
    sqlx::query(
        r#"INSERT INTO "Cotisation" ("id", "guildId", "userId", "propre", "sale", "description", "date", "createdById", "createdAt")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(guild_id)
    .bind(&new.user_id)
    .bind(new.propre)
    .bind(new.sale)
    .bind(&new.description)
    .bind(new.date)
    .bind(&new.created_by_id)
    .bind(now)
    .execute(db)
    .await?;
    Ok(Cotisation {
        id,
        user_id: new.user_id,
        propre: new.propre,
        sale: new.sale,
        description: new.description,
        date: new.date,
        created_by_id: new.created_by_id,
    })
}

pub async fn get(db: &SqlitePool, guild_id: &str, id: &str) -> Result<Option<Cotisation>> {
    Ok(sqlx::query_as(&format!(r#"SELECT {COLUMNS} FROM "Cotisation" WHERE "guildId" = ? AND "id" = ?"#))
        .bind(guild_id)
        .bind(id)
        .fetch_optional(db)
        .await?)
}

pub async fn delete(db: &SqlitePool, guild_id: &str, id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "Cotisation" WHERE "guildId" = ? AND "id" = ?"#)
        .bind(guild_id)
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Cotisations du filtre, les plus récentes d'abord.
pub async fn list(
    db: &SqlitePool,
    guild_id: &str,
    filter: &Filter,
    limit: i64,
    offset: i64,
) -> Result<Vec<Cotisation>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Cotisation" WHERE {WHERE} ORDER BY "date" DESC, "createdAt" DESC LIMIT ?5 OFFSET ?6"#
    ))
    .bind(guild_id)
    .bind(filter.since)
    .bind(filter.until)
    .bind(&filter.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await?)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub count: i64,
    pub propre: i64,
    pub sale: i64,
}

impl Totals {
    pub fn total(&self) -> i64 {
        self.propre.saturating_add(self.sale)
    }
}

pub async fn totals(db: &SqlitePool, guild_id: &str, filter: &Filter) -> Result<Totals> {
    let (count, propre, sale): (i64, i64, i64) = sqlx::query_as(&format!(
        r#"SELECT COUNT(*), COALESCE(SUM("propre"), 0), COALESCE(SUM("sale"), 0) FROM "Cotisation" WHERE {WHERE}"#
    ))
    .bind(guild_id)
    .bind(filter.since)
    .bind(filter.until)
    .bind(&filter.user_id)
    .fetch_one(db)
    .await?;
    Ok(Totals { count, propre, sale })
}

/// Totaux par membre, du plus gros contributeur au plus petit.
pub async fn by_member(db: &SqlitePool, guild_id: &str, filter: &Filter) -> Result<Vec<(String, Totals)>> {
    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(&format!(
        r#"SELECT "userId", COUNT(*), SUM("propre"), SUM("sale") FROM "Cotisation" WHERE {WHERE}
           GROUP BY "userId" ORDER BY SUM("propre") + SUM("sale") DESC, "userId""#
    ))
    .bind(guild_id)
    .bind(filter.since)
    .bind(filter.until)
    .bind(&filter.user_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(user, count, propre, sale)| (user, Totals { count, propre, sale })).collect())
}

pub async fn channel(db: &SqlitePool, guild_id: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as(r#"SELECT "channelId" FROM "CotisationConfig" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    Ok(row.and_then(|(channel,)| channel))
}

pub async fn set_channel(db: &SqlitePool, guild_id: &str, channel_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "CotisationConfig" ("guildId", "channelId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "channelId" = excluded."channelId", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn totals_by_period_and_member() {
        let dir = std::env::temp_dir().join(format!("cotisation-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let day = |days_ago: i64| DbTime(Utc::now() - Duration::days(days_ago));
        for (user, propre, sale, days_ago) in
            [("a", 1000, 0, 1), ("a", 0, 5000, 10), ("b", 200, 300, 2), ("c", 50, 0, 40)]
        {
            add(
                &db,
                "g",
                NewCotisation {
                    user_id: user.into(),
                    propre,
                    sale,
                    description: "test".into(),
                    date: day(days_ago),
                    created_by_id: user.into(),
                },
            )
            .await
            .unwrap();
        }
        add(
            &db,
            "autre",
            NewCotisation {
                user_id: "a".into(),
                propre: 9999,
                sale: 0,
                description: "x".into(),
                date: day(1),
                created_by_id: "a".into(),
            },
        )
        .await
        .unwrap();

        let all = totals(&db, "g", &Filter::default()).await.unwrap();
        assert_eq!(all, Totals { count: 4, propre: 1250, sale: 5300 });

        let week = Filter { since: Some(day(7)), ..Default::default() };
        assert_eq!(totals(&db, "g", &week).await.unwrap(), Totals { count: 2, propre: 1200, sale: 300 });

        let members = by_member(&db, "g", &Filter::default()).await.unwrap();
        assert_eq!(members.iter().map(|(u, _)| u.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(members[0].1.total(), 6000);

        let only_b = Filter { user_id: Some("b".into()), ..Default::default() };
        let rows = list(&db, "g", &only_b, 10, 0).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(delete(&db, "g", &rows[0].id).await.unwrap());
        assert_eq!(totals(&db, "g", &only_b).await.unwrap().count, 0);

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
