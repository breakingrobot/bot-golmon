use super::{
    embed::{build_buttons, build_dashboard, build_embed},
    model::{Status, Transaction, TransactionConfig, TransactionItem},
};
use crate::{
    context::Ctx,
    db::is_foreign_key_violation,
    discord::{
        messages::{self, Mentions, Outgoing, parse_id},
        site::{self, Page},
        suggest,
    },
    modules::like_contains,
    util::{ids::new_id, time::DbTime},
};
use anyhow::Result;
use sqlx::SqlitePool;

/// Nombre de transactions terminées listées par statut dans le tableau de bord.
const TERMINAL_DASHBOARD_LIMIT: i64 = 10;

const COLUMNS: &str =
    r#""id", "channelId", "messageId", "groupe", "type", "prix", "statut", "createdById", "updatedAt""#;

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Option<TransactionConfig>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "TransactionConfig" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_optional(db)
        .await?)
}

/// Installe le tableau de bord dans un nouveau salon : l'ancien message n'est plus pertinent.
pub async fn set_dashboard_channel(db: &SqlitePool, guild_id: &str, channel_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "TransactionConfig" ("guildId", "dashboardChannelId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "dashboardChannelId" = excluded."dashboardChannelId", "dashboardMessageId" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_ping_role(db: &SqlitePool, guild_id: &str, role_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "TransactionConfig" ("guildId", "pingRoleId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "pingRoleId" = excluded."pingRoleId", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(role_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

async fn set_dashboard_message(db: &SqlitePool, guild_id: &str, message_id: &str) -> Result<()> {
    sqlx::query(r#"UPDATE "TransactionConfig" SET "dashboardMessageId" = ?, "updatedAt" = ? WHERE "guildId" = ?"#)
        .bind(message_id)
        .bind(DbTime::now())
        .bind(guild_id)
        .execute(db)
        .await?;
    Ok(())
}

pub struct NewTransaction {
    pub guild_id: String,
    pub channel_id: String,
    pub groupe: String,
    pub kind: String,
    pub prix: f64,
    pub created_by_id: String,
}

pub async fn create(db: &SqlitePool, params: NewTransaction) -> Result<String> {
    let id = new_id();
    let now = DbTime::now();
    sqlx::query(
        r#"INSERT INTO "Transaction" ("id", "guildId", "channelId", "groupe", "type", "prix", "statut", "createdById", "createdAt", "updatedAt")
           VALUES (?, ?, ?, ?, ?, ?, 'PREPARATION', ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(&params.guild_id)
    .bind(&params.channel_id)
    .bind(&params.groupe)
    .bind(&params.kind)
    .bind(params.prix)
    .bind(&params.created_by_id)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;
    Ok(id)
}

/// Ajoute un objet. `Ok(false)` si la transaction a disparu entre-temps (supprimée
/// pendant que le formulaire était ouvert).
pub async fn add_item(
    db: &SqlitePool,
    transaction_id: &str,
    nom: &str,
    quantite: i64,
    prix_unitaire: Option<f64>,
) -> Result<bool> {
    let now = DbTime::now();
    let result = sqlx::query(
        r#"INSERT INTO "TransactionItem" ("id", "transactionId", "nom", "quantite", "prixUnitaire", "createdAt") VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(new_id())
    .bind(transaction_id)
    .bind(nom)
    .bind(quantite)
    .bind(prix_unitaire)
    .bind(now)
    .execute(db)
    .await;
    match result {
        Ok(_) => {}
        Err(error) if is_foreign_key_violation(&error) => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    sqlx::query(r#"UPDATE "Transaction" SET "updatedAt" = ? WHERE "id" = ?"#)
        .bind(now)
        .bind(transaction_id)
        .execute(db)
        .await?;
    Ok(true)
}

pub async fn set_status(db: &SqlitePool, guild_id: &str, transaction_id: &str, status: Status) -> Result<bool> {
    let result =
        sqlx::query(r#"UPDATE "Transaction" SET "statut" = ?, "updatedAt" = ? WHERE "id" = ? AND "guildId" = ?"#)
            .bind(status.as_str())
            .bind(DbTime::now())
            .bind(transaction_id)
            .bind(guild_id)
            .execute(db)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete(db: &SqlitePool, guild_id: &str, transaction_id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "Transaction" WHERE "id" = ? AND "guildId" = ?"#)
        .bind(transaction_id)
        .bind(guild_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

async fn attach_items(db: &SqlitePool, transactions: &mut [Transaction]) -> Result<()> {
    if transactions.is_empty() {
        return Ok(());
    }
    let placeholders = vec!["?"; transactions.len()].join(", ");
    let sql = format!(
        r#"SELECT "transactionId", "nom", "quantite", "prixUnitaire" FROM "TransactionItem" WHERE "transactionId" IN ({placeholders}) ORDER BY "createdAt" ASC"#
    );
    let mut query = sqlx::query_as::<_, TransactionItem>(&sql);
    for transaction in transactions.iter() {
        query = query.bind(&transaction.id);
    }
    let items = query.fetch_all(db).await?;
    for transaction in transactions.iter_mut() {
        transaction.items = items.iter().filter(|item| item.transaction_id == transaction.id).cloned().collect();
    }
    Ok(())
}

/// Toujours scoper à la guilde : l'identifiant d'une transaction est public (il
/// figure dans le pied de son embed) et l'autocomplétion Discord n'empêche pas
/// de saisir une valeur arbitraire. Sans ce filtre, n'importe qui pourrait lire
/// ou modifier la transaction d'un autre serveur.
pub async fn get(db: &SqlitePool, guild_id: &str, transaction_id: &str) -> Result<Option<Transaction>> {
    let transaction: Option<Transaction> =
        sqlx::query_as(&format!(r#"SELECT {COLUMNS} FROM "Transaction" WHERE "id" = ? AND "guildId" = ?"#))
            .bind(transaction_id)
            .bind(guild_id)
            .fetch_optional(db)
            .await?;
    let Some(mut transaction) = transaction else {
        return Ok(None);
    };
    attach_items(db, std::slice::from_mut(&mut transaction)).await?;
    Ok(Some(transaction))
}

pub async fn search(db: &SqlitePool, guild_id: &str, query: &str, open_only: bool) -> Result<Vec<Transaction>> {
    let filter = if open_only { r#"AND "statut" NOT IN ('ANNULEE', 'CLOTUREE')"# } else { "" };
    Ok(sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Transaction" WHERE "guildId" = ? AND "groupe" LIKE ? ESCAPE '\' {filter} ORDER BY "createdAt" DESC LIMIT 25"#
    ))
    .bind(guild_id)
    .bind(like_contains(query))
    .fetch_all(db)
    .await?)
}

/// Groupes déjà utilisés, du plus récent au plus ancien.
pub async fn known_groups(db: &SqlitePool, guild_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT "groupe" FROM "Transaction" WHERE "guildId" = ? GROUP BY "groupe" ORDER BY MAX("createdAt") DESC LIMIT 200"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(groupe,)| groupe).collect())
}

/// Objets déjà saisis dans les transactions, les plus fréquents d'abord.
pub async fn known_items(db: &SqlitePool, guild_id: &str) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT i."nom" FROM "TransactionItem" i JOIN "Transaction" t ON t."id" = i."transactionId"
           WHERE t."guildId" = ? GROUP BY i."nom" ORDER BY COUNT(*) DESC, MAX(i."createdAt") DESC LIMIT 200"#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(nom,)| nom).collect())
}

/// Choix du groupe : contacts du répertoire (avec leur relation) puis groupes
/// déjà utilisés, sans doublon.
pub fn group_choices(contacts: Vec<(String, &'static str)>, used: Vec<String>, typed: &str) -> Vec<(String, String)> {
    let contacts = contacts.into_iter().map(|(name, emoji)| (format!("{emoji} {name}"), name));
    suggest::filter(contacts.chain(used.into_iter().map(|g| (g.clone(), g))), typed)
}

pub async fn sync_message(ctx: &Ctx, guild_id: &str, transaction_id: &str) -> Result<()> {
    let Some(transaction) = get(&ctx.db, guild_id, transaction_id).await? else {
        return Ok(());
    };
    let Some(channel_id) = parse_id(&transaction.channel_id) else {
        return Ok(());
    };

    let mut message = Outgoing::embeds(vec![build_embed(&transaction)])
        .with_components(build_buttons(&transaction.id, transaction.status()));

    if let Some(message_id) = transaction.message_id.as_deref().and_then(parse_id)
        && messages::fetch(ctx, channel_id, message_id).await?.is_some()
    {
        messages::edit(ctx, channel_id, message_id, &message).await?;
        return Ok(());
    }
    // Message supprimé : on en publie un nouveau ci-dessous.

    // Le rôle n'est mentionné qu'à la toute première publication. Republier après
    // une suppression manuelle ne doit pas re-pinguer tout le rôle à chaque action.
    if transaction.message_id.is_none()
        && let Some(role_id) = get_config(&ctx.db, guild_id).await?.and_then(|c| c.ping_role_id)
    {
        message.content = Some(format!("<@&{role_id}>"));
        if let Some(role) = parse_id(&role_id) {
            message.mentions = Mentions::Explicit { users: Vec::new(), roles: vec![role], everyone: false };
        }
    }

    let sent = messages::send(ctx, channel_id, &message).await?;
    sqlx::query(r#"UPDATE "Transaction" SET "messageId" = ? WHERE "id" = ?"#)
        .bind(sent.id.to_string())
        .bind(&transaction.id)
        .execute(&ctx.db)
        .await?;
    Ok(())
}

/// Met à jour (ou crée) le message unique du tableau de bord. Les transactions
/// en cours sont listées intégralement ; les terminées sont bornées aux plus
/// récentes, sans quoi le tableau de bord grossirait jusqu'à dépasser les
/// limites d'un message.
pub async fn sync_dashboard(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let Some(config) = get_config(&ctx.db, guild_id).await? else {
        return Ok(());
    };
    let Some(channel_id) = config.dashboard_channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };

    let mut transactions: Vec<Transaction> = sqlx::query_as(&format!(
        r#"SELECT {COLUMNS} FROM "Transaction" WHERE "guildId" = ? AND "statut" IN ('PREPARATION', 'EN_ATTENTE_RDV', 'AVANCE_ATTENTE_PAIEMENT') ORDER BY "updatedAt" DESC"#
    ))
    .bind(guild_id)
    .fetch_all(&ctx.db)
    .await?;
    for status in Status::TERMINAL {
        let bucket: Vec<Transaction> = sqlx::query_as(&format!(
            r#"SELECT {COLUMNS} FROM "Transaction" WHERE "guildId" = ? AND "statut" = ? ORDER BY "updatedAt" DESC LIMIT ?"#
        ))
        .bind(guild_id)
        .bind(status.as_str())
        .bind(TERMINAL_DASHBOARD_LIMIT)
        .fetch_all(&ctx.db)
        .await?;
        transactions.extend(bucket);
    }
    attach_items(&ctx.db, &mut transactions).await?;
    let (total,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "Transaction" WHERE "guildId" = ?"#)
        .bind(guild_id)
        .fetch_one(&ctx.db)
        .await?;

    let mut components = build_dashboard(&transactions, total);
    site::attach_page(ctx, guild_id, Page::Transactions { groupe: None }, &mut components);
    let message = Outgoing::components_v2(components);
    let known = config.dashboard_message_id.as_deref().and_then(parse_id);
    match messages::upsert(ctx, channel_id, known, &message).await {
        Ok(Some(new_id)) => set_dashboard_message(&ctx.db, guild_id, &new_id.to_string()).await,
        Ok(None) => Ok(()),
        Err(error) => {
            tracing::warn!(guild = guild_id, ?error, "tableau de bord des transactions inaccessible");
            Ok(())
        }
    }
}

/// Rafraîchit l'embed de la transaction et le tableau de bord de la guilde.
pub async fn refresh(ctx: &Ctx, guild_id: &str, transaction_id: &str) -> Result<()> {
    sync_message(ctx, guild_id, transaction_id).await?;
    sync_dashboard(ctx, guild_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_from_directory_then_history() {
        let contacts = vec![("Ballas".to_owned(), "🔴"), ("Garage Benny's".to_owned(), "🟢")];
        let used = vec!["ballas".to_owned(), "Vagos".to_owned()];
        let choices = group_choices(contacts.clone(), used.clone(), "");
        let labels: Vec<&str> = choices.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["🔴 Ballas", "🟢 Garage Benny's", "Vagos"], "« ballas » déjà proposé par le répertoire");
        assert_eq!(group_choices(contacts, used, "vag"), [("Vagos".to_owned(), "Vagos".to_owned())]);
    }

    #[tokio::test]
    async fn remembers_groups_and_items() {
        let dir = std::env::temp_dir().join(format!("transaction-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        for groupe in ["Ballas", "Vagos", "Ballas"] {
            let new = NewTransaction {
                guild_id: "g".into(),
                channel_id: "c".into(),
                groupe: groupe.into(),
                kind: "VENTE".into(),
                prix: 10.0,
                created_by_id: "u".into(),
            };
            let id = create(&db, new).await.unwrap();
            add_item(&db, &id, "Pistolet", 1, None).await.unwrap();
        }
        let mut groups = known_groups(&db, "g").await.unwrap();
        groups.sort();
        assert_eq!(groups, ["Ballas", "Vagos"]);
        assert_eq!(known_items(&db, "g").await.unwrap(), ["Pistolet"]);
        assert!(known_groups(&db, "autre").await.unwrap().is_empty());
        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
