//! Accès au répertoire des contacts (commun à Discord et au site) et message
//! permanent de l'annuaire.

use super::{Kind, Relation};
use crate::{
    context::Ctx,
    db::is_unique_violation,
    discord::{
        components::{container, separator, text_display, text_displays},
        messages::{self, Outgoing, parse_id},
    },
    modules::like_contains,
    util::{
        format::{ListBlock, V2_TEXT_LIMIT, pack_blocks, plural, truncate},
        ids::new_id,
        time::DbTime,
    },
};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use twilight_model::channel::message::Component;

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub relation: String,
    pub phone: Option<String>,
    pub description: Option<String>,
    pub created_by_id: String,
    pub created_at: DbTime,
    pub updated_at: DbTime,
}

impl Contact {
    /// Valeur inconnue en base (modifiée à la main) : « Groupe ».
    pub fn kind(&self) -> Kind {
        Kind::parse(&self.kind).unwrap_or(Kind::Groupe)
    }

    /// Valeur inconnue en base : « Neutre ».
    pub fn relation(&self) -> Relation {
        Relation::parse(&self.relation).unwrap_or(Relation::Neutre)
    }
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub contact_id: String,
    pub author_id: String,
    pub body: String,
    pub created_at: DbTime,
}

pub struct NewContact {
    pub name: String,
    pub kind: Kind,
    pub relation: Relation,
    pub phone: Option<String>,
    pub description: Option<String>,
    pub created_by_id: String,
}

/// Filtres de la liste : relation, type, texte cherché dans le nom ou la description.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub relation: Option<Relation>,
    pub kind: Option<Kind>,
    pub query: Option<String>,
}

const COLUMNS: &str =
    r#""id", "name", "kind", "relation", "phone", "description", "createdById", "createdAt", "updatedAt""#;
/// Alliés, neutres puis hostiles ; par nom dans chaque groupe.
const ORDER: &str = r#"CASE "relation" WHEN 'ALLIE' THEN 0 WHEN 'NEUTRE' THEN 1 ELSE 2 END, "name" COLLATE NOCASE"#;

/// `Ok(None)` : un contact porte déjà ce nom (sans tenir compte de la casse).
pub async fn add(db: &SqlitePool, guild_id: &str, new: NewContact) -> Result<Option<Contact>> {
    let contact = Contact {
        id: new_id(),
        name: new.name,
        kind: new.kind.as_str().to_owned(),
        relation: new.relation.as_str().to_owned(),
        phone: new.phone,
        description: new.description,
        created_by_id: new.created_by_id,
        created_at: DbTime::now(),
        updated_at: DbTime::now(),
    };
    let result = sqlx::query(
        r#"INSERT INTO "Contact" ("id", "guildId", "name", "kind", "relation", "phone", "description", "createdById", "createdAt", "updatedAt")
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&contact.id)
    .bind(guild_id)
    .bind(&contact.name)
    .bind(&contact.kind)
    .bind(&contact.relation)
    .bind(&contact.phone)
    .bind(&contact.description)
    .bind(&contact.created_by_id)
    .bind(contact.created_at)
    .bind(contact.updated_at)
    .execute(db)
    .await;
    match result {
        Ok(_) => Ok(Some(contact)),
        Err(error) if is_unique_violation(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Enregistre les champs modifiés d'un contact. `Ok(false)` : nom déjà pris.
pub async fn save(db: &SqlitePool, guild_id: &str, contact: &Contact) -> Result<bool> {
    let result = sqlx::query(
        r#"UPDATE "Contact" SET "name" = ?, "kind" = ?, "relation" = ?, "phone" = ?, "description" = ?, "updatedAt" = ?
           WHERE "guildId" = ? AND "id" = ?"#,
    )
    .bind(&contact.name)
    .bind(&contact.kind)
    .bind(&contact.relation)
    .bind(&contact.phone)
    .bind(&contact.description)
    .bind(DbTime::now())
    .bind(guild_id)
    .bind(&contact.id)
    .execute(db)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(error) if is_unique_violation(&error) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Supprime un contact et ses notes (cascade).
pub async fn delete(db: &SqlitePool, guild_id: &str, id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "Contact" WHERE "guildId" = ? AND "id" = ?"#)
        .bind(guild_id)
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Contact choisi dans l'autocomplétion (identifiant) ou tapé à la main (nom).
pub async fn resolve(db: &SqlitePool, guild_id: &str, value: &str) -> Result<Option<Contact>> {
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Contact" WHERE "guildId" = ?1 AND ("id" = ?2 OR "name" = ?2 COLLATE NOCASE)
           ORDER BY "id" = ?2 DESC LIMIT 1"#
    ))
    .bind(guild_id)
    .bind(value.trim())
    .fetch_optional(db)
    .await?)
}

pub async fn list(db: &SqlitePool, guild_id: &str, filter: &Filter) -> Result<Vec<Contact>> {
    let query = filter.query.as_deref().map(str::trim).filter(|q| !q.is_empty()).map(like_contains);
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Contact"
           WHERE "guildId" = ?1 AND (?2 IS NULL OR "relation" = ?2) AND (?3 IS NULL OR "kind" = ?3)
             AND (?4 IS NULL OR "name" LIKE ?4 ESCAPE '\' OR "description" LIKE ?4 ESCAPE '\')
           ORDER BY {ORDER}"#
    ))
    .bind(guild_id)
    .bind(filter.relation.map(Relation::as_str))
    .bind(filter.kind.map(Kind::as_str))
    .bind(query)
    .fetch_all(db)
    .await?)
}

pub async fn add_note(db: &SqlitePool, guild_id: &str, contact_id: &str, author_id: &str, body: &str) -> Result<Note> {
    let note = Note {
        id: new_id(),
        contact_id: contact_id.to_owned(),
        author_id: author_id.to_owned(),
        body: body.to_owned(),
        created_at: DbTime::now(),
    };
    sqlx::query(
        r#"INSERT INTO "ContactNote" ("id", "contactId", "guildId", "authorId", "body", "createdAt") VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&note.id)
    .bind(contact_id)
    .bind(guild_id)
    .bind(author_id)
    .bind(body)
    .bind(note.created_at)
    .execute(db)
    .await?;
    Ok(note)
}

const NOTE_COLUMNS: &str = r#""id", "contactId", "authorId", "body", "createdAt""#;

/// Dernières notes d'un contact, les plus récentes d'abord, et leur nombre total.
pub async fn notes(db: &SqlitePool, guild_id: &str, contact_id: &str, limit: i64) -> Result<(Vec<Note>, i64)> {
    let (total,): (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "ContactNote" WHERE "guildId" = ? AND "contactId" = ?"#)
            .bind(guild_id)
            .bind(contact_id)
            .fetch_one(db)
            .await?;
    let rows = sqlx::query_as(&format!(
        r#"SELECT {NOTE_COLUMNS} FROM "ContactNote" WHERE "guildId" = ? AND "contactId" = ?
           ORDER BY "createdAt" DESC, rowid DESC LIMIT ?"#
    ))
    .bind(guild_id)
    .bind(contact_id)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok((rows, total))
}

/// Toutes les notes du serveur par contact, les plus récentes d'abord (site).
pub async fn notes_by_contact(db: &SqlitePool, guild_id: &str) -> Result<HashMap<String, Vec<Note>>> {
    let rows: Vec<Note> = sqlx::query_as(&format!(
        r#"SELECT {NOTE_COLUMNS} FROM "ContactNote" WHERE "guildId" = ? ORDER BY "createdAt" DESC, rowid DESC"#
    ))
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    let mut map: HashMap<String, Vec<Note>> = HashMap::new();
    for note in rows {
        map.entry(note.contact_id.clone()).or_default().push(note);
    }
    Ok(map)
}

#[derive(Clone, Debug, Default, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct ContactConfig {
    pub channel_id: Option<String>,
    pub message_id: Option<String>,
}

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<ContactConfig> {
    Ok(sqlx::query_as(r#"SELECT "channelId", "messageId" FROM "ContactConfig" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_optional(db)
        .await?
        .unwrap_or_default())
}

/// Change le salon de l'annuaire (`None` : plus d'annuaire). Le message connu
/// est oublié : l'appelant le supprime, puis l'annuaire est republié.
pub async fn set_channel(db: &SqlitePool, guild_id: &str, channel_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "ContactConfig" ("guildId", "channelId", "messageId", "updatedAt") VALUES (?, ?, NULL, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "channelId" = excluded."channelId", "messageId" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Ligne de l'annuaire : jamais de note du staff.
pub fn directory_line(contact: &Contact) -> String {
    let kind = contact.kind();
    let mut line = format!("- {} **{}**", kind.emoji(), contact.name);
    if let Some(phone) = &contact.phone {
        line.push_str(&format!(" · `{phone}`"));
    }
    if let Some(description) = &contact.description {
        line.push_str(&format!(" — {}", truncate(&description.replace('\n', " "), 120)));
    }
    line
}

/// Titre et pied de page sont décomptés à part ; on garde une marge.
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 300;
const ACCENT_COLOR: u32 = 0x5865f2;

/// Annuaire en Components V2, groupé par relation.
pub fn build_directory(contacts: &[Contact]) -> Vec<Component> {
    let footer = format!("-# Détail et téléphone : `/contact voir` · mis à jour <t:{}:R>", DbTime::now().unix());
    let mut components = Vec::new();
    if contacts.is_empty() {
        components.push(text_display("## 📇 Répertoire des contacts\nAucun contact enregistré."));
    } else {
        let header = format!(
            "## 📇 Répertoire des contacts\n**{}** contact{s} enregistré{s}",
            contacts.len(),
            s = plural(contacts.len())
        );
        let blocks: Vec<ListBlock> = Relation::ALL
            .iter()
            .map(|relation| {
                let lines: Vec<String> =
                    contacts.iter().filter(|c| c.relation() == *relation).map(directory_line).collect();
                ListBlock {
                    header: format!("### {} {} ({})", relation.emoji(), relation.plural_label(), lines.len()),
                    lines,
                    empty: "*Aucun.*".to_owned(),
                }
            })
            .collect();
        let budget = TEXT_BUDGET.saturating_sub(header.len() + footer.len());
        components.push(text_display(header));
        for block in pack_blocks(&blocks, budget) {
            components.push(separator());
            components.extend(text_displays(&block));
        }
    }
    components.push(separator());
    components.push(text_display(footer));
    vec![container(Some(ACCENT_COLOR), components)]
}

/// Met à jour (ou publie) l'annuaire s'il est configuré. Une erreur Discord est
/// tracée sans faire échouer l'action qui l'a déclenchée.
pub async fn sync_directory(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let config = get_config(&ctx.db, guild_id).await?;
    let Some(channel_id) = config.channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let message = Outgoing::components_v2(build_directory(&list(&ctx.db, guild_id, &Filter::default()).await?));
    let known = config.message_id.as_deref().and_then(parse_id);
    match messages::upsert(ctx, channel_id, known, &message).await {
        Ok(Some(new_id)) => {
            sqlx::query(r#"UPDATE "ContactConfig" SET "messageId" = ?, "updatedAt" = ? WHERE "guildId" = ?"#)
                .bind(new_id.to_string())
                .bind(DbTime::now())
                .bind(guild_id)
                .execute(&ctx.db)
                .await?;
        }
        Ok(None) => {}
        Err(error) => tracing::error!(guild = guild_id, ?error, "synchronisation du répertoire des contacts échouée"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        util::format::V2_COMPONENT_LIMIT,
    };

    fn new(name: &str, kind: Kind, relation: Relation, description: Option<&str>) -> NewContact {
        NewContact {
            name: name.into(),
            kind,
            relation,
            phone: None,
            description: description.map(str::to_owned),
            created_by_id: "1".into(),
        }
    }

    #[tokio::test]
    async fn contacts_are_unique_filtered_and_cascade() {
        let dir = std::env::temp_dir().join(format!("contact-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();

        let ballas = add(&db, "g", new("Les Ballas", Kind::Groupe, Relation::Hostile, Some("Gang de Grove")))
            .await
            .unwrap()
            .unwrap();
        assert!(
            add(&db, "g", new("les BALLAS", Kind::Personne, Relation::Allie, None)).await.unwrap().is_none(),
            "nom unique sans tenir compte de la casse"
        );
        assert!(add(&db, "autre", new("Les Ballas", Kind::Groupe, Relation::Allie, None)).await.unwrap().is_some());
        let garage = add(&db, "g", new("Garage Benny's", Kind::Entreprise, Relation::Allie, Some("Pièces auto")))
            .await
            .unwrap()
            .unwrap();
        add(&db, "g", new("Tony", Kind::Personne, Relation::Neutre, Some("Indic au port"))).await.unwrap().unwrap();

        // Ordre : alliés, neutres, hostiles.
        let all = list(&db, "g", &Filter::default()).await.unwrap();
        assert_eq!(all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Garage Benny's", "Tony", "Les Ballas"]);
        let hostile = Filter { relation: Some(Relation::Hostile), ..Default::default() };
        assert_eq!(list(&db, "g", &hostile).await.unwrap().len(), 1);
        let people = Filter { kind: Some(Kind::Personne), ..Default::default() };
        assert_eq!(list(&db, "g", &people).await.unwrap()[0].name, "Tony");
        let search = Filter { query: Some("PORT".into()), ..Default::default() };
        assert_eq!(list(&db, "g", &search).await.unwrap()[0].name, "Tony");
        let joker = Filter { query: Some("%".into()), ..Default::default() };
        assert!(list(&db, "g", &joker).await.unwrap().is_empty(), "jokers LIKE échappés");

        assert_eq!(resolve(&db, "g", "garage benny's").await.unwrap().unwrap().id, garage.id);
        assert_eq!(resolve(&db, "g", &ballas.id).await.unwrap().unwrap().name, "Les Ballas");
        assert!(resolve(&db, "autre", &garage.id).await.unwrap().is_none());

        // Renommer vers un nom pris est refusé.
        let mut renamed = garage.clone();
        renamed.name = "TONY".into();
        assert!(!save(&db, "g", &renamed).await.unwrap());
        renamed.name = "Garage Benny".into();
        renamed.relation = Relation::Hostile.as_str().into();
        assert!(save(&db, "g", &renamed).await.unwrap());
        assert_eq!(resolve(&db, "g", &garage.id).await.unwrap().unwrap().relation(), Relation::Hostile);

        // Notes : dernières d'abord, supprimées avec le contact.
        for body in ["première", "deuxième", "troisième"] {
            add_note(&db, "g", &ballas.id, "9", body).await.unwrap();
        }
        let (latest, total) = notes(&db, "g", &ballas.id, 2).await.unwrap();
        assert_eq!(total, 3);
        assert_eq!(latest.iter().map(|n| n.body.as_str()).collect::<Vec<_>>(), ["troisième", "deuxième"]);
        assert_eq!(notes_by_contact(&db, "g").await.unwrap()[&ballas.id].len(), 3);
        assert!(delete(&db, "g", &ballas.id).await.unwrap());
        let (_, total) = notes(&db, "g", &ballas.id, 10).await.unwrap();
        assert_eq!(total, 0, "notes supprimées en cascade");
        let (orphans,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "ContactNote""#).fetch_one(&db).await.unwrap();
        assert_eq!(orphans, 0);

        // Configuration de l'annuaire.
        assert!(get_config(&db, "g").await.unwrap().channel_id.is_none());
        set_channel(&db, "g", Some("42")).await.unwrap();
        assert_eq!(get_config(&db, "g").await.unwrap().channel_id.as_deref(), Some("42"));
        set_channel(&db, "g", None).await.unwrap();
        assert!(get_config(&db, "g").await.unwrap().channel_id.is_none());

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }

    fn contact(index: usize, relation: Relation) -> Contact {
        Contact {
            id: index.to_string(),
            name: format!("Contact n°{index}"),
            kind: Kind::Groupe.as_str().into(),
            relation: relation.as_str().into(),
            phone: Some("555-1234".into()),
            description: Some("Une description publique assez longue pour remplir l'annuaire. ".repeat(4)),
            created_by_id: "1".into(),
            created_at: DbTime::now(),
            updated_at: DbTime::now(),
        }
    }

    #[test]
    fn directory_groups_by_relation_within_limits() {
        let components = build_directory(&[contact(1, Relation::Hostile), contact(2, Relation::Allie)]);
        let text = v2_text(&components);
        assert!(text.contains("**2** contacts enregistrés"));
        let allies = text.find("### 🟢 Alliés (1)").unwrap();
        let neutral = text.find("### ⚪ Neutres (0)").unwrap();
        let hostile = text.find("### 🔴 Hostiles (1)").unwrap();
        assert!(allies < neutral && neutral < hostile);
        assert!(text.contains("- 👥 **Contact n°2** · `555-1234` — Une description"));

        assert!(v2_text(&build_directory(&[])).contains("Aucun contact enregistré."));

        let many: Vec<Contact> = (0..300).map(|i| contact(i, Relation::ALL[i % 3])).collect();
        let components = build_directory(&many);
        let (text_len, count) = v2_usage(&components);
        assert!(text_len <= V2_TEXT_LIMIT, "{text_len} octets");
        assert!(count <= V2_COMPONENT_LIMIT, "{count} composants");
        let text = v2_text(&components);
        assert!(text.contains("### 🔴 Hostiles (100)") && text.contains("autres"));
    }
}
