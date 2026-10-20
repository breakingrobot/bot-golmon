//! Modèles de transaction (`transaction.modeles`) : type, groupe, prix et
//! objets habituels, pour pré-remplir `/transaction nouvelle`.

use crate::util::{ids::new_id, time::DbTime};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};

/// Modèles au plus par serveur (une autocomplétion Discord en montre 25).
pub const MAX_TEMPLATES: i64 = 25;
pub const NAME_LIMIT: usize = 60;

#[derive(Clone, Debug, PartialEq, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub nom: String,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub groupe: Option<String>,
    pub prix: Option<f64>,
    /// Liste canonique (`items::format_items`).
    pub objets: String,
}

pub struct NewTemplate {
    pub nom: String,
    pub kind: String,
    pub groupe: Option<String>,
    pub prix: Option<f64>,
    pub objets: String,
    pub created_by_id: String,
}

const COLUMNS: &str = r#""id", "nom", "type", "groupe", "prix", "objets""#;

pub async fn list(db: &SqlitePool, guild_id: &str) -> Result<Vec<Template>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "TransactionModele" WHERE "guildId" = ? ORDER BY "nom" COLLATE NOCASE"#
    ))
    .bind(guild_id)
    .fetch_all(db)
    .await?)
}

/// Modèle choisi par l'autocomplétion (identifiant) ou tapé (nom exact, sans casse).
pub async fn resolve(db: &SqlitePool, guild_id: &str, value: &str) -> Result<Option<Template>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "TransactionModele" WHERE "guildId" = ?1 AND ("id" = ?2 OR "nom" = ?2 COLLATE NOCASE)
           ORDER BY "id" = ?2 DESC LIMIT 1"#
    ))
    .bind(guild_id)
    .bind(value.trim())
    .fetch_optional(db)
    .await?)
}

/// Pourquoi un modèle n'a pas été créé.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    NameTaken,
    TooMany,
}

pub async fn create(db: &SqlitePool, guild_id: &str, new: NewTemplate) -> Result<Result<Template, Refused>> {
    let (count,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "TransactionModele" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_one(db)
        .await?;
    if count >= MAX_TEMPLATES {
        return Ok(Err(Refused::TooMany));
    }
    let id = new_id();
    let result = sqlx::query(
        r#"INSERT OR IGNORE INTO "TransactionModele" ("id", "guildId", "nom", "type", "groupe", "prix", "objets", "createdById", "createdAt")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(guild_id)
    .bind(&new.nom)
    .bind(&new.kind)
    .bind(&new.groupe)
    .bind(new.prix)
    .bind(&new.objets)
    .bind(&new.created_by_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(Err(Refused::NameTaken));
    }
    Ok(Ok(Template { id, nom: new.nom, kind: new.kind, groupe: new.groupe, prix: new.prix, objets: new.objets }))
}

pub async fn delete(db: &SqlitePool, guild_id: &str, id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "TransactionModele" WHERE "guildId" = ? AND "id" = ?"#)
        .bind(guild_id)
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(nom: &str) -> NewTemplate {
        NewTemplate {
            nom: nom.into(),
            kind: "VENTE".into(),
            groupe: Some("Ballas".into()),
            prix: None,
            objets: "2x Pistolet @ 15 000".into(),
            created_by_id: "u".into(),
        }
    }

    #[tokio::test]
    async fn creates_resolves_and_deletes() {
        let dir = std::env::temp_dir().join(format!("transaction-modeles-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let template = create(&db, "g", new("Armes Ballas")).await.unwrap().unwrap();
        assert_eq!(create(&db, "g", new("armes ballas")).await.unwrap(), Err(Refused::NameTaken));
        assert!(create(&db, "autre", new("Armes Ballas")).await.unwrap().is_ok(), "par serveur");
        assert_eq!(resolve(&db, "g", "ARMES BALLAS").await.unwrap(), Some(template.clone()));
        assert_eq!(resolve(&db, "g", &template.id).await.unwrap(), Some(template.clone()));
        assert_eq!(resolve(&db, "autre", &template.id).await.unwrap(), None);
        assert_eq!(list(&db, "g").await.unwrap().len(), 1);
        assert!(!delete(&db, "autre", &template.id).await.unwrap());
        assert!(delete(&db, "g", &template.id).await.unwrap());
        for i in 0..MAX_TEMPLATES {
            create(&db, "g", new(&format!("m{i}"))).await.unwrap().unwrap();
        }
        assert_eq!(create(&db, "g", new("de trop")).await.unwrap(), Err(Refused::TooMany));
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
