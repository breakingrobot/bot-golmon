//! Mouvements des coffres, stock calculé, noms des propriétés et liens joueur → membre.

use super::parser::{Kind, Movement};
use crate::util::time::DbTime;
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};

/// Enregistre un mouvement ; `false` s'il était déjà connu (message relu).
pub async fn insert(
    db: &SqlitePool,
    id: &str,
    guild_id: &str,
    channel_id: &str,
    movement: &Movement,
    at: DbTime,
) -> Result<bool> {
    let result = sqlx::query(
        r#"INSERT OR IGNORE INTO "CoffreMovement"
           ("id", "guildId", "source", "propertyId", "chest", "item", "amount", "kind", "playerName", "playerGameId", "at", "channelId")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(id)
    .bind(guild_id)
    .bind(&movement.source)
    .bind(&movement.property_id)
    .bind(movement.chest)
    .bind(&movement.item)
    .bind(movement.amount)
    .bind(movement.kind.as_str())
    .bind(&movement.player_name)
    .bind(&movement.player_game_id)
    .bind(at)
    .bind(channel_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() > 0)
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct MovementRow {
    pub property_id: String,
    pub chest: Option<i64>,
    pub item: String,
    pub amount: i64,
    pub kind: String,
    pub player_name: String,
    pub player_game_id: Option<String>,
    pub at: DbTime,
}

impl MovementRow {
    pub fn kind(&self) -> Kind {
        if self.kind == Kind::Withdraw.as_str() { Kind::Withdraw } else { Kind::Deposit }
    }
}

/// Filtre commun aux commandes et au site.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub property_id: Option<String>,
    pub item: Option<String>,
    pub player_game_ids: Option<Vec<String>>,
    pub kind: Option<Kind>,
    pub since: Option<DbTime>,
}

impl Filter {
    fn sql(&self) -> (String, Vec<String>) {
        let mut clauses = vec![r#""guildId" = ?"#.to_owned()];
        let mut binds = Vec::new();
        if let Some(property) = &self.property_id {
            clauses.push(r#""propertyId" = ?"#.to_owned());
            binds.push(property.clone());
        }
        if let Some(item) = &self.item {
            clauses.push(r#""item" = ?"#.to_owned());
            binds.push(item.clone());
        }
        if let Some(players) = &self.player_game_ids {
            if players.is_empty() {
                clauses.push("0".to_owned());
            } else {
                clauses.push(format!(r#""playerGameId" IN ({})"#, vec!["?"; players.len()].join(", ")));
                binds.extend(players.iter().cloned());
            }
        }
        if let Some(kind) = self.kind {
            clauses.push(r#""kind" = ?"#.to_owned());
            binds.push(kind.as_str().to_owned());
        }
        if let Some(since) = self.since {
            clauses.push(r#""at" >= ?"#.to_owned());
            binds.push(since.to_db_string());
        }
        (clauses.join(" AND "), binds)
    }
}

pub async fn movements(
    db: &SqlitePool,
    guild_id: &str,
    filter: &Filter,
    limit: i64,
    offset: i64,
) -> Result<Vec<MovementRow>> {
    let (clause, binds) = filter.sql();
    let sql = format!(
        r#"SELECT "propertyId", "chest", "item", "amount", "kind", "playerName", "playerGameId", "at"
           FROM "CoffreMovement" WHERE {clause} ORDER BY "at" DESC, "id" DESC LIMIT ? OFFSET ?"#
    );
    let mut query = sqlx::query_as::<_, MovementRow>(&sql).bind(guild_id);
    for bind in &binds {
        query = query.bind(bind);
    }
    Ok(query.bind(limit).bind(offset).fetch_all(db).await?)
}

pub async fn count(db: &SqlitePool, guild_id: &str, filter: &Filter) -> Result<i64> {
    let (clause, binds) = filter.sql();
    let sql = format!(r#"SELECT COUNT(*) FROM "CoffreMovement" WHERE {clause}"#);
    let mut query = sqlx::query_as::<_, (i64,)>(&sql).bind(guild_id);
    for bind in &binds {
        query = query.bind(bind);
    }
    Ok(query.fetch_one(db).await?.0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockLine {
    pub property_id: String,
    pub item: String,
    pub deposited: i64,
    pub withdrawn: i64,
}

impl StockLine {
    /// Solde des mouvements connus (dépôts − retraits).
    pub fn quantity(&self) -> i64 {
        self.deposited - self.withdrawn
    }
}

/// Stock par propriété et par objet, d'après les mouvements enregistrés.
pub async fn stock(db: &SqlitePool, guild_id: &str, property_id: Option<&str>) -> Result<Vec<StockLine>> {
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
        r#"SELECT "propertyId", "item",
                  COALESCE(SUM(CASE WHEN "kind" = 'DEPOSIT' THEN "amount" END), 0),
                  COALESCE(SUM(CASE WHEN "kind" = 'WITHDRAW' THEN "amount" END), 0)
           FROM "CoffreMovement" WHERE "guildId" = ?1 AND (?2 IS NULL OR "propertyId" = ?2)
           GROUP BY "propertyId", "item" ORDER BY "propertyId", "item""#,
    )
    .bind(guild_id)
    .bind(property_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(property_id, item, deposited, withdrawn)| StockLine { property_id, item, deposited, withdrawn })
        .collect())
}

/// Activité par joueur : (identifiant, nom, dépôts, retraits, mouvements).
pub async fn by_player(
    db: &SqlitePool,
    guild_id: &str,
    filter: &Filter,
) -> Result<Vec<(String, String, i64, i64, i64)>> {
    let (clause, binds) = filter.sql();
    let sql = format!(
        r#"SELECT COALESCE("playerGameId", "playerName"), MAX("playerName"),
                  COALESCE(SUM(CASE WHEN "kind" = 'DEPOSIT' THEN "amount" END), 0),
                  COALESCE(SUM(CASE WHEN "kind" = 'WITHDRAW' THEN "amount" END), 0),
                  COUNT(*)
           FROM "CoffreMovement" WHERE {clause}
           GROUP BY COALESCE("playerGameId", "playerName") ORDER BY COUNT(*) DESC"#
    );
    let mut query = sqlx::query_as::<_, (String, String, i64, i64, i64)>(&sql).bind(guild_id);
    for bind in &binds {
        query = query.bind(bind);
    }
    Ok(query.fetch_all(db).await?)
}

/// Propriétés connues (vues dans les mouvements ou nommées) avec leur nom.
pub async fn properties(db: &SqlitePool, guild_id: &str) -> Result<Vec<(String, Option<String>)>> {
    Ok(sqlx::query_as(
        r#"SELECT p."propertyId", n."name" FROM
             (SELECT DISTINCT "propertyId" FROM "CoffreMovement" WHERE "guildId" = ?1
              UNION SELECT "propertyId" FROM "CoffreProperty" WHERE "guildId" = ?1) p
           LEFT JOIN "CoffreProperty" n ON n."guildId" = ?1 AND n."propertyId" = p."propertyId"
           ORDER BY COALESCE(n."name", p."propertyId")"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?)
}

pub async fn name_property(db: &SqlitePool, guild_id: &str, property_id: &str, name: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "CoffreProperty" ("guildId", "propertyId", "name") VALUES (?, ?, ?)
           ON CONFLICT ("guildId", "propertyId") DO UPDATE SET "name" = excluded."name""#,
    )
    .bind(guild_id)
    .bind(property_id)
    .bind(name)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn link_player(db: &SqlitePool, guild_id: &str, game_id: &str, user_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "CoffrePlayer" ("guildId", "gameId", "userId") VALUES (?, ?, ?)
           ON CONFLICT ("guildId", "gameId") DO UPDATE SET "userId" = excluded."userId""#,
    )
    .bind(guild_id)
    .bind(game_id)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Identifiants de jeu reliés à un membre.
pub async fn game_ids_of(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as(r#"SELECT "gameId" FROM "CoffrePlayer" WHERE "guildId" = ? AND "userId" = ?"#)
            .bind(guild_id)
            .bind(user_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Liens joueur → membre du serveur.
pub async fn player_links(db: &SqlitePool, guild_id: &str) -> Result<std::collections::HashMap<String, String>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as(r#"SELECT "gameId", "userId" FROM "CoffrePlayer" WHERE "guildId" = ?"#)
            .bind(guild_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Objets connus, pour l'autocomplétion.
pub async fn items(db: &SqlitePool, guild_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as(r#"SELECT DISTINCT "item" FROM "CoffreMovement" WHERE "guildId" = ? ORDER BY "item""#)
            .bind(guild_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().map(|(item,)| item).collect())
}

pub async fn channels(db: &SqlitePool, guild_id: &str) -> Result<Vec<(String, Option<String>)>> {
    Ok(sqlx::query_as(r#"SELECT "channelId", "lastMessageId" FROM "CoffreChannel" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_all(db)
        .await?)
}

pub async fn is_channel(db: &SqlitePool, channel_id: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(r#"SELECT "guildId" FROM "CoffreChannel" WHERE "channelId" = ?"#)
        .bind(channel_id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(guild,)| guild))
}

pub async fn add_channel(db: &SqlitePool, guild_id: &str, channel_id: &str) -> Result<()> {
    sqlx::query(r#"INSERT OR IGNORE INTO "CoffreChannel" ("channelId", "guildId") VALUES (?, ?)"#)
        .bind(channel_id)
        .bind(guild_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn remove_channel(db: &SqlitePool, guild_id: &str, channel_id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "CoffreChannel" WHERE "guildId" = ? AND "channelId" = ?"#)
        .bind(guild_id)
        .bind(channel_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Retient le message le plus récent lu dans le salon (identifiants croissants).
pub async fn mark_read(db: &SqlitePool, channel_id: &str, message_id: u64) -> Result<()> {
    sqlx::query(
        r#"UPDATE "CoffreChannel" SET "lastMessageId" = ?2
           WHERE "channelId" = ?1 AND ("lastMessageId" IS NULL OR CAST("lastMessageId" AS INTEGER) < ?3)"#,
    )
    .bind(channel_id)
    .bind(message_id.to_string())
    .bind(message_id as i64)
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::coffres::parser::Movement;

    fn movement(item: &str, amount: i64, kind: Kind, player: &str) -> Movement {
        Movement {
            source: "Coffre propriété".into(),
            property_id: "8740".into(),
            chest: Some(1),
            item: item.into(),
            amount,
            kind,
            player_name: format!("Joueur {player}"),
            player_game_id: Some(player.into()),
            at: None,
        }
    }

    #[tokio::test]
    async fn stock_history_and_dedupe() {
        let dir = std::env::temp_dir().join(format!("coffre-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let now = DbTime::now();
        let rows = [
            ("m1:0", movement("repair_kit", 8, Kind::Deposit, "1")),
            ("m2:0", movement("repair_kit", 3, Kind::Withdraw, "2")),
            ("m3:0", movement("lockpick", 2, Kind::Deposit, "2")),
        ];
        for (id, m) in &rows {
            assert!(insert(&db, id, "g", "c", m, now).await.unwrap());
        }
        // Message relu (rattrapage) : ignoré.
        assert!(!insert(&db, "m1:0", "g", "c", &rows[0].1, now).await.unwrap());

        let stock = stock(&db, "g", None).await.unwrap();
        let kit = stock.iter().find(|l| l.item == "repair_kit").unwrap();
        assert_eq!((kit.deposited, kit.withdrawn, kit.quantity()), (8, 3, 5));

        link_player(&db, "g", "2", "u2").await.unwrap();
        let ids = game_ids_of(&db, "g", "u2").await.unwrap();
        let filter = Filter { player_game_ids: Some(ids), ..Default::default() };
        assert_eq!(count(&db, "g", &filter).await.unwrap(), 2);
        let withdrawals = Filter { kind: Some(Kind::Withdraw), ..Default::default() };
        assert_eq!(movements(&db, "g", &withdrawals, 10, 0).await.unwrap()[0].amount, 3);
        let nobody = Filter { player_game_ids: Some(Vec::new()), ..Default::default() };
        assert_eq!(count(&db, "g", &nobody).await.unwrap(), 0);

        name_property(&db, "g", "8740", "Planque").await.unwrap();
        assert_eq!(properties(&db, "g").await.unwrap(), vec![("8740".to_owned(), Some("Planque".to_owned()))]);

        add_channel(&db, "g", "c").await.unwrap();
        mark_read(&db, "c", 200).await.unwrap();
        mark_read(&db, "c", 100).await.unwrap();
        assert_eq!(channels(&db, "g").await.unwrap(), vec![("c".to_owned(), Some("200".to_owned()))]);

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
