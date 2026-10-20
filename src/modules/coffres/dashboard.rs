//! Tableau de bord permanent du stock des coffres (Components V2), par
//! propriété, avec les objets sous leur seuil signalés. Mis à jour après de
//! nouveaux mouvements, au plus une fois par minute et par serveur : le jeu peut
//! publier des dizaines de mouvements d'affilée.

use super::{
    alerts::{self, Thresholds},
    commands::property_label,
    parser::item_label,
    service::{self, StockLine},
};
use crate::{
    context::{Context, Ctx},
    discord::{
        components::{container, separator, text_display, text_displays},
        messages::{self, Outgoing, parse_id},
        site::{self, Page},
    },
    util::{
        format::{ListBlock, V2_TEXT_LIMIT, pack_blocks, plural},
        time::DbTime,
    },
};
use anyhow::Result;
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};
use twilight_model::channel::message::Component;

const ACCENT_COLOR: u32 = 0xe67e22;
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 300;
/// Intervalle minimal entre deux mises à jour du tableau d'un serveur.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
/// Propriétés affichées au plus (les autres sont résumées).
const MAX_PROPERTIES: usize = 15;

/// Salon et message du tableau de bord.
pub async fn get_dashboard(db: &SqlitePool, guild_id: &str) -> Result<(Option<String>, Option<String>)> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT "dashboardChannelId", "dashboardMessageId" FROM "CoffreAlertConfig" WHERE "guildId" = ?"#,
    )
    .bind(guild_id)
    .fetch_optional(db)
    .await?;
    Ok(row.unwrap_or_default())
}

/// Change le salon du tableau (`None` : plus de tableau). Le message connu est
/// oublié : l'appelant supprime l'ancien, puis le tableau est republié.
pub async fn set_dashboard_channel(db: &SqlitePool, guild_id: &str, channel_id: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "CoffreAlertConfig" ("guildId", "dashboardChannelId", "dashboardMessageId", "updatedAt") VALUES (?, ?, NULL, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "dashboardChannelId" = excluded."dashboardChannelId",
             "dashboardMessageId" = NULL, "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(channel_id)
    .bind(DbTime::now())
    .execute(db)
    .await?;
    Ok(())
}

/// Ligne d'un objet : quantité, et seuil s'il est en stock bas.
fn item_line(line: &StockLine, thresholds: &Thresholds) -> String {
    let quantity = line.quantity();
    match thresholds.min_for(&line.property_id, &line.item) {
        Some(min) if quantity < min => {
            format!("- ⚠️ {} × **{quantity}** *(seuil {min})*", item_label(&line.item))
        }
        _ => format!("- {} × **{quantity}**", item_label(&line.item)),
    }
}

/// Tableau du stock : objets épuisés masqués, sauf ceux sous un seuil.
pub fn build(lines: &[StockLine], names: &HashMap<String, String>, thresholds: &Thresholds) -> Vec<Component> {
    let mut grouped: BTreeMap<String, Vec<&StockLine>> = BTreeMap::new();
    for line in lines {
        if line.quantity() != 0 || thresholds.is_low(&line.property_id, &line.item, line.quantity()) {
            grouped.entry(line.property_id.clone()).or_default().push(line);
        }
    }
    let low = lines.iter().filter(|l| thresholds.is_low(&l.property_id, &l.item, l.quantity())).count();
    let footer = format!(
        "-# Calculé à partir des mouvements lus depuis le début du suivi · mis à jour <t:{}:R>",
        DbTime::now().unix()
    );
    let mut header = "## 📦 Stock des coffres".to_owned();
    if !thresholds.is_empty() {
        header.push_str(&if low == 0 {
            "\n✅ Aucun objet sous son seuil.".to_owned()
        } else {
            format!("\n⚠️ **{low}** objet{s} sous le seuil", s = plural(low))
        });
    }
    let mut components = vec![text_display(header.clone())];
    if grouped.is_empty() {
        components.push(text_display("Aucun objet en stock d'après les mouvements enregistrés."));
    } else {
        // Propriétés avec un objet bas d'abord, puis par nom.
        let mut properties: Vec<(String, Vec<&StockLine>)> = grouped.into_iter().collect();
        properties.sort_by_key(|(id, lines)| {
            let has_low = lines.iter().any(|l| thresholds.is_low(&l.property_id, &l.item, l.quantity()));
            (!has_low, property_label(names, id).to_lowercase())
        });
        let hidden = properties.len().saturating_sub(MAX_PROPERTIES);
        let blocks: Vec<ListBlock> = properties
            .iter()
            .take(MAX_PROPERTIES)
            .map(|(id, lines)| ListBlock {
                header: format!("### {}", property_label(names, id)),
                lines: lines.iter().map(|l| item_line(l, thresholds)).collect(),
                empty: "*Vide.*".to_owned(),
            })
            .collect();
        let mut budget = TEXT_BUDGET.saturating_sub(header.len() + footer.len());
        if hidden > 0 {
            budget = budget.saturating_sub(80);
        }
        for block in pack_blocks(&blocks, budget) {
            components.push(separator());
            components.extend(text_displays(&block));
        }
        if hidden > 0 {
            components.push(text_display(format!(
                "… et {hidden} autre{s} propriété{s} (`/coffre stock`)",
                s = plural(hidden)
            )));
        }
    }
    components.push(separator());
    components.push(text_display(footer));
    vec![container(Some(ACCENT_COLOR), components)]
}

/// Met à jour (ou publie) le tableau du stock s'il est configuré.
pub async fn sync_dashboard(ctx: &Ctx, guild_id: &str) -> Result<()> {
    let (channel, known) = get_dashboard(&ctx.db, guild_id).await?;
    let Some(channel_id) = channel.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let names: HashMap<String, String> = service::properties(&ctx.db, guild_id)
        .await?
        .into_iter()
        .filter_map(|(id, name)| name.map(|name| (id, name)))
        .collect();
    let lines = service::stock(&ctx.db, guild_id, None).await?;
    let thresholds = alerts::thresholds(&ctx.db, guild_id).await?;
    let mut components = build(&lines, &names, &thresholds);
    site::attach_page(ctx, guild_id, Page::Coffres { propriete: None, objet: None }, &mut components);
    let message = Outgoing::components_v2(components);
    match messages::upsert(ctx, channel_id, known.as_deref().and_then(parse_id), &message).await {
        Ok(Some(new_id)) => {
            sqlx::query(r#"UPDATE "CoffreAlertConfig" SET "dashboardMessageId" = ? WHERE "guildId" = ?"#)
                .bind(new_id.to_string())
                .bind(guild_id)
                .execute(&ctx.db)
                .await?;
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(guild = guild_id, ?error, "tableau du stock des coffres inaccessible"),
    }
    Ok(())
}

#[derive(Default)]
struct Slot {
    last: Option<Instant>,
    scheduled: bool,
}

static SLOTS: LazyLock<Mutex<HashMap<String, Slot>>> = LazyLock::new(Default::default);

/// Attente avant la prochaine mise à jour, d'après la dernière.
pub fn delay_after(last: Option<Instant>, now: Instant) -> Duration {
    last.map_or(Duration::ZERO, |last| MIN_INTERVAL.saturating_sub(now.saturating_duration_since(last)))
}

/// Demande une mise à jour du tableau : immédiate si la dernière date d'au
/// moins une minute, sinon groupée avec les suivantes à la fin de la minute.
pub fn request_refresh(ctx: &Context, guild_id: &str) {
    let delay = {
        let mut slots = SLOTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let slot = slots.entry(guild_id.to_owned()).or_default();
        if slot.scheduled {
            return;
        }
        slot.scheduled = true;
        delay_after(slot.last, Instant::now())
    };
    let ctx = Arc::clone(ctx);
    let guild = guild_id.to_owned();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        {
            // Libéré avant la mise à jour : un mouvement arrivé pendant celle-ci en programme une autre.
            let mut slots = SLOTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let slot = slots.entry(guild.clone()).or_default();
            slot.scheduled = false;
            slot.last = Some(Instant::now());
        }
        if let Err(error) = sync_dashboard(&ctx, &guild).await {
            tracing::warn!(guild, ?error, "mise à jour du tableau du stock impossible");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        modules::coffres::alerts::Threshold,
        util::format::V2_COMPONENT_LIMIT,
    };

    fn line(property: &str, item: &str, deposited: i64, withdrawn: i64) -> StockLine {
        StockLine { property_id: property.into(), item: item.into(), deposited, withdrawn, adjusted: 0 }
    }

    #[test]
    fn flags_low_items_and_stays_within_limits() {
        let thresholds = Thresholds::new(&[Threshold { property_id: None, item: "kit".into(), min: 5 }]);
        let mut lines = vec![line("1", "kit", 3, 0), line("1", "pain", 4, 4), line("2", "kit", 10, 0)];
        let names: HashMap<String, String> = [("2".to_owned(), "Planque".to_owned())].into();
        let text = v2_text(&build(&lines, &names, &thresholds));
        assert!(text.contains("⚠️ **1** objet sous le seuil"));
        assert!(text.contains("*(seuil 5)*"));
        assert!(!text.contains("pain"), "objet épuisé sans seuil masqué");
        assert!(text.find("Propriété 1").unwrap() < text.find("Planque").unwrap(), "propriété en alerte d'abord");

        for p in 0..40 {
            for i in 0..30 {
                lines.push(line(&format!("p{p}"), &format!("objet_{i}_avec_un_nom_long"), 100, 1));
            }
        }
        let components = build(&lines, &names, &thresholds);
        let (text, count) = v2_usage(&components);
        assert!(text <= V2_TEXT_LIMIT, "{text}");
        assert!(count <= V2_COMPONENT_LIMIT, "{count}");
        assert!(v2_text(&components).contains("autres propriétés"));
    }

    #[test]
    fn refresh_delay_respects_interval() {
        let now = Instant::now();
        assert_eq!(delay_after(None, now), Duration::ZERO);
        assert_eq!(delay_after(Some(now), now), MIN_INTERVAL);
        let old = now.checked_sub(Duration::from_secs(90)).unwrap_or(now);
        if old != now {
            assert_eq!(delay_after(Some(old), now), Duration::ZERO);
        }
    }
}
