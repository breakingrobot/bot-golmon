use super::{NAMESPACE, format};
use crate::{
    context::Ctx,
    db::is_unique_violation,
    discord::{
        components::{button, row},
        embed::{EmbedBuilder, footer, now},
        messages::{self, Outgoing, parse_id},
    },
    util::{
        format::{FIELD_LIMIT, field, len16, mention, pack_fields},
        ids::{encode_custom_id, new_id},
        time::DbTime,
    },
};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};
use twilight_model::channel::message::{Component, Embed, component::ButtonStyle};

pub const NUMBER_TAKEN: &str = "Ce numéro est déjà attribué à un autre membre.";

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PhoneNumber {
    pub user_id: String,
    pub numero: String,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct PhoneConfig {
    pub dashboard_channel_id: Option<String>,
    pub dashboard_message_id: Option<String>,
}

pub async fn get_config(db: &SqlitePool, guild_id: &str) -> Result<Option<PhoneConfig>> {
    Ok(sqlx::query_as(r#"SELECT * FROM "PhoneConfig" WHERE "guildId" = ?"#).bind(guild_id).fetch_optional(db).await?)
}

pub async fn set_dashboard_channel(db: &SqlitePool, guild_id: &str, channel_id: &str) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "PhoneConfig" ("guildId", "dashboardChannelId", "updatedAt") VALUES (?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "dashboardChannelId" = excluded."dashboardChannelId", "dashboardMessageId" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Un numéro par membre (le nouveau remplace l'ancien) et unique dans la
/// guilde. `Ok(false)` : numéro déjà attribué à quelqu'un d'autre.
pub async fn set(db: &SqlitePool, guild_id: &str, user_id: &str, numero: &str) -> Result<bool> {
    let now = DbTime::now();
    let result = sqlx::query(
        r#"INSERT INTO "PhoneNumber" ("id", "guildId", "userId", "numero", "createdAt", "updatedAt") VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT ("guildId", "userId") DO UPDATE SET "numero" = excluded."numero", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(new_id())
    .bind(guild_id)
    .bind(user_id)
    .bind(numero)
    .bind(now)
    .bind(now)
    .execute(db)
    .await;
    match result {
        Ok(_) => Ok(true),
        Err(error) if is_unique_violation(&error) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub async fn remove(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<bool> {
    let result = sqlx::query(r#"DELETE FROM "PhoneNumber" WHERE "guildId" = ? AND "userId" = ?"#)
        .bind(guild_id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn get(db: &SqlitePool, guild_id: &str, user_id: &str) -> Result<Option<PhoneNumber>> {
    Ok(sqlx::query_as(r#"SELECT "userId", "numero" FROM "PhoneNumber" WHERE "guildId" = ? AND "userId" = ?"#)
        .bind(guild_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?)
}

pub async fn list(db: &SqlitePool, guild_id: &str) -> Result<Vec<PhoneNumber>> {
    Ok(sqlx::query_as(r#"SELECT "userId", "numero" FROM "PhoneNumber" WHERE "guildId" = ? ORDER BY "numero" ASC"#)
        .bind(guild_id)
        .fetch_all(db)
        .await?)
}

pub fn buttons(guild_id: &str) -> Component {
    row(vec![
        Component::Button(button(
            encode_custom_id(NAMESPACE, "add", guild_id, None),
            "📱 Ajouter / modifier mon numéro",
            ButtonStyle::Primary,
        )),
        Component::Button(button(
            encode_custom_id(NAMESPACE, "remove", guild_id, None),
            "🗑️ Retirer mon numéro",
            ButtonStyle::Danger,
        )),
    ])
}

const FIELD_BUDGET: usize = 5200;
const MAX_FIELDS: usize = 24;

pub fn build_dashboard(numbers: &[PhoneNumber]) -> Embed {
    let builder = EmbedBuilder::new().title("📱 Annuaire téléphonique").color(0x5865f2).timestamp(now());
    if numbers.is_empty() {
        return builder
            .description("Aucun numéro enregistré. Ajoute le tien avec le bouton ci-dessous ou `/numero ajouter`.")
            .build();
    }

    // Regroupe les lignes en champs d'au plus 1024 caractères.
    let mut groups: Vec<Vec<String>> = Vec::new();
    for entry in numbers {
        let line = format!("`{}` — {}", format(&entry.numero), mention(&entry.user_id));
        match groups.last_mut() {
            Some(group) if len16(&group.join("\n")) + len16(&line) < FIELD_LIMIT => group.push(line),
            _ => groups.push(vec![line]),
        }
    }
    let total = groups.len();
    let fields = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let name = if total > 1 { format!("Numéros ({}/{total})", index + 1) } else { "Numéros".to_owned() };
            field(name, group.join("\n"), false)
        })
        .collect();

    let (kept, dropped) = pack_fields(fields, FIELD_BUDGET, MAX_FIELDS);
    let mut builder = builder;
    for f in kept {
        builder = builder.field(f);
    }
    if !dropped.is_empty() {
        let hidden: usize = dropped.iter().map(|f| f.value.lines().count()).sum();
        builder =
            builder.field(field("…", format!("+{hidden} numéro(s) non affiché(s) — utilise `/numero voir`."), false));
    }
    builder.footer(footer(format!("{} numéro(s) enregistré(s)", numbers.len()))).build()
}

pub async fn sync_dashboard(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let Some(config) = get_config(&ctx.db, guild_id).await? else {
        return Ok(());
    };
    let Some(channel_id) = config.dashboard_channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let message = Outgoing::embeds(vec![build_dashboard(&list(&ctx.db, guild_id).await?)])
        .with_components(vec![buttons(guild_id)]);
    let known = config.dashboard_message_id.as_deref().and_then(parse_id);
    match messages::upsert(ctx, channel_id, known, &message).await {
        Ok(Some(new_id)) => {
            sqlx::query(r#"UPDATE "PhoneConfig" SET "dashboardMessageId" = ?, "updatedAt" = ? WHERE "guildId" = ?"#)
                .bind(new_id.to_string())
                .bind(DbTime::now())
                .bind(guild_id)
                .execute(&ctx.db)
                .await?;
        }
        Ok(None) => {}
        Err(error) => tracing::error!(guild = guild_id, ?error, "synchronisation de l'annuaire échouée"),
    }
    Ok(())
}
