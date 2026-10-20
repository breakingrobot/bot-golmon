use super::{NAMESPACE, format};
use crate::{
    context::Ctx,
    db::is_unique_violation,
    discord::{
        components::{button, container, section, separator, text_display, text_displays},
        messages::{self, Outgoing, parse_id},
        site::{self, Page},
    },
    util::{
        format::{V2_TEXT_LIMIT, and_others, bounded_lines, mention, plural},
        ids::{encode_custom_id, new_id},
        time::DbTime,
    },
};
use anyhow::Result;
use sqlx::{FromRow, SqlitePool};
use twilight_model::channel::message::{Component, component::ButtonStyle};

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

/// Les deux actions de l'annuaire, chacune à côté de son bouton. Les
/// `custom_id` sont ceux des anciens messages : leurs boutons restent valides.
fn actions(guild_id: &str) -> Vec<Component> {
    vec![
        section(
            vec!["Ajoute ton numéro à l'annuaire, ou mets-le à jour.".to_owned()],
            Component::Button(button(
                encode_custom_id(NAMESPACE, "add", guild_id, None),
                "📱 Ajouter / modifier mon numéro",
                ButtonStyle::Primary,
            )),
        ),
        section(
            vec!["Retire ton numéro de l'annuaire.".to_owned()],
            Component::Button(button(
                encode_custom_id(NAMESPACE, "remove", guild_id, None),
                "🗑️ Retirer mon numéro",
                ButtonStyle::Danger,
            )),
        ),
    ]
}

/// Titre, actions et pied de page sont décomptés à part ; on garde une marge.
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 300;
const ACCENT_COLOR: u32 = 0x5865f2;

/// Annuaire en Components V2 : la liste des numéros, puis les boutons d'action.
pub fn build_dashboard(guild_id: &str, numbers: &[PhoneNumber]) -> Vec<Component> {
    let footer = format!("-# Mis à jour <t:{}:R>", DbTime::now().unix());
    let mut components = Vec::new();
    if numbers.is_empty() {
        components.push(text_display(
            "## 📱 Annuaire téléphonique\nAucun numéro enregistré. Ajoute le tien avec le bouton ci-dessous ou `/numero ajouter`.",
        ));
    } else {
        let header = format!(
            "## 📱 Annuaire téléphonique\n**{}** numéro{s} enregistré{s}",
            numbers.len(),
            s = plural(numbers.len())
        );
        let lines: Vec<String> = numbers
            .iter()
            .map(|entry| format!("- `{}` — {}", format(&entry.numero), mention(&entry.user_id)))
            .collect();
        let list = bounded_lines(&lines, TEXT_BUDGET.saturating_sub(header.len() + footer.len()), |hidden| {
            format!("{} — utilise `/numero voir`.", and_others(hidden))
        });
        components.extend([text_display(header), separator()]);
        components.extend(text_displays(&list));
    }
    components.push(separator());
    components.extend(actions(guild_id));
    components.push(text_display(footer));
    vec![container(Some(ACCENT_COLOR), components)]
}

pub async fn sync_dashboard(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let Some(config) = get_config(&ctx.db, guild_id).await? else {
        return Ok(());
    };
    let Some(channel_id) = config.dashboard_channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let mut components = build_dashboard(guild_id, &list(&ctx.db, guild_id).await?);
    site::attach_page(ctx, guild_id, Page::Annuaire, &mut components);
    let message = Outgoing::components_v2(components);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        util::format::V2_COMPONENT_LIMIT,
    };

    fn number(index: u64) -> PhoneNumber {
        PhoneNumber { user_id: (100_000_000_000_000_000 + index).to_string(), numero: format!("555{index:04}") }
    }

    /// Les boutons gardent les `custom_id` routés par `interactions::button`.
    fn custom_ids(components: &[Component]) -> Vec<String> {
        components
            .iter()
            .flat_map(|component| match component {
                Component::Container(container) => custom_ids(&container.components),
                Component::Section(section) => custom_ids(std::slice::from_ref(&section.accessory)),
                Component::Button(button) => button.custom_id.clone().into_iter().collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    #[test]
    fn dashboard_lists_numbers_and_keeps_buttons() {
        let components = build_dashboard("7", &[number(1), number(2)]);
        let text = v2_text(&components);
        assert!(text.starts_with("## 📱 Annuaire téléphonique\n**2** numéros enregistrés"));
        assert!(text.contains("- `555-0001` — <@100000000000000001>\n- `555-0002` — <@100000000000000002>"));
        assert_eq!(
            custom_ids(&components),
            [encode_custom_id(NAMESPACE, "add", "7", None), encode_custom_id(NAMESPACE, "remove", "7", None)]
        );

        let empty = build_dashboard("7", &[]);
        assert!(v2_text(&empty).contains("Aucun numéro enregistré"));
        assert_eq!(custom_ids(&empty).len(), 2);
    }

    #[test]
    fn dashboard_stays_within_v2_limits() {
        let numbers: Vec<PhoneNumber> = (0..500).map(number).collect();
        let components = build_dashboard("7", &numbers);
        let (text_len, count) = v2_usage(&components);
        assert!(text_len <= V2_TEXT_LIMIT, "{text_len} octets");
        assert!(count <= V2_COMPONENT_LIMIT, "{count} composants");
        let text = v2_text(&components);
        assert!(text.contains("**500** numéros enregistrés"));
        assert!(text.contains("autres — utilise `/numero voir`."));
        assert_eq!(custom_ids(&components).len(), 2);
    }
}
