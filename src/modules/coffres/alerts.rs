//! Alertes des coffres : stock d'un objet qui passe sous un seuil, gros retrait,
//! retrait par un personnage relié à aucun membre. Un message par lot de
//! nouveaux mouvements (message en direct ou rattrapage), qui ne mentionne que
//! le rôle configuré. Le premier import de l'historique d'un salon n'alerte pas.

use super::{
    commands::property_label,
    parser::{Kind, Movement, item_label},
    service::{self, Recorded},
};
use crate::{
    context::Ctx,
    discord::{
        embed::{EmbedBuilder, footer},
        messages::{self, Mentions, Outgoing, parse_id},
    },
    util::{
        format::{DESCRIPTION_LIMIT, bounded_lines, plural},
        time::{DbTime, format_date_label},
    },
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

const COLOR: u32 = 0xe74c3c;
/// Quantité maximale d'un seuil ou d'un gros retrait (comme les mouvements lus).
pub const MAX_QUANTITY: i64 = 1_000_000;
/// Nombre maximal de seuils par serveur.
pub const MAX_THRESHOLDS: usize = 200;

/// Seuil de stock bas d'un objet, dans une propriété ou dans toutes (`None`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Threshold {
    pub property_id: Option<String>,
    pub item: String,
    pub min: i64,
}

/// Réglages des alertes, tels qu'échangés avec le site (`CoffreAlerts` côté TypeScript).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CoffreAlerts {
    /// Salon des alertes, `None` : désactivées.
    pub channel_id: Option<String>,
    /// Rôle mentionné dans les alertes.
    pub role_id: Option<String>,
    pub thresholds: Vec<Threshold>,
    /// Un retrait unique d'au moins cette quantité déclenche une alerte.
    pub big_withdrawal: Option<i64>,
    /// Alerte quand un personnage relié à aucun membre retire un objet.
    pub unknown_player: bool,
}

fn is_snowflake(raw: &str) -> bool {
    !raw.is_empty() && raw.len() <= 20 && raw.bytes().all(|b| b.is_ascii_digit())
}

impl CoffreAlerts {
    /// Vérifie une saisie ; `Err` porte le message pour l'utilisateur.
    pub fn validate(&self) -> Result<(), String> {
        if self.channel_id.as_deref().is_some_and(|c| !is_snowflake(c)) {
            return Err("Salon invalide.".to_owned());
        }
        if self.role_id.as_deref().is_some_and(|r| !is_snowflake(r)) {
            return Err("Rôle invalide.".to_owned());
        }
        if self.big_withdrawal.is_some_and(|q| !(1..=MAX_QUANTITY).contains(&q)) {
            return Err(format!("Le seuil de gros retrait doit être entre 1 et {MAX_QUANTITY}."));
        }
        if self.thresholds.len() > MAX_THRESHOLDS {
            return Err(format!("{MAX_THRESHOLDS} seuils au plus."));
        }
        for threshold in &self.thresholds {
            if threshold.item.trim().is_empty() || threshold.item.chars().count() > 100 {
                return Err("Objet invalide dans un seuil.".to_owned());
            }
            if threshold.property_id.as_deref().is_some_and(|p| p.trim().is_empty() || p.chars().count() > 50) {
                return Err("Propriété invalide dans un seuil.".to_owned());
            }
            if !(1..=MAX_QUANTITY).contains(&threshold.min) {
                return Err(format!("Un seuil doit être entre 1 et {MAX_QUANTITY}."));
            }
        }
        Ok(())
    }
}

/// Seuils d'un serveur, pour signaler les objets en stock bas.
#[derive(Clone, Debug, Default)]
pub struct Thresholds {
    /// (propriété, objet) → minimum ; propriété vide = toutes.
    by_key: HashMap<(String, String), i64>,
}

impl Thresholds {
    pub fn new(list: &[Threshold]) -> Self {
        Self {
            by_key: list.iter().map(|t| ((t.property_id.clone().unwrap_or_default(), t.item.clone()), t.min)).collect(),
        }
    }

    /// Minimum applicable : celui de la propriété, sinon celui de toutes les propriétés.
    pub fn min_for(&self, property_id: &str, item: &str) -> Option<i64> {
        self.by_key
            .get(&(property_id.to_owned(), item.to_owned()))
            .or_else(|| self.by_key.get(&(String::new(), item.to_owned())))
            .copied()
    }

    /// Stock bas : sous le seuil applicable.
    pub fn is_low(&self, property_id: &str, item: &str, quantity: i64) -> bool {
        self.min_for(property_id, item).is_some_and(|min| quantity < min)
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

/// Seuils du serveur, triés par objet puis propriété (toutes d'abord).
pub async fn list_thresholds(db: &SqlitePool, guild_id: &str) -> Result<Vec<Threshold>> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"SELECT "propertyId", "item", "min" FROM "CoffreThreshold" WHERE "guildId" = ? ORDER BY "item", "propertyId""#,
    )
    .bind(guild_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(property, item, min)| Threshold { property_id: (!property.is_empty()).then_some(property), item, min })
        .collect())
}

/// Seuils du serveur, prêts à signaler les objets en stock bas (`low`).
pub async fn thresholds(db: &SqlitePool, guild_id: &str) -> Result<Thresholds> {
    Ok(Thresholds::new(&list_thresholds(db, guild_id).await?))
}

/// Pose (ou retire, avec `min` = 0) un seuil. Renvoie `false` si un retrait ne
/// visait aucun seuil existant.
pub async fn set_threshold(
    db: &SqlitePool,
    guild_id: &str,
    property_id: Option<&str>,
    item: &str,
    min: i64,
) -> Result<bool> {
    let property = property_id.unwrap_or_default();
    if min <= 0 {
        let result =
            sqlx::query(r#"DELETE FROM "CoffreThreshold" WHERE "guildId" = ? AND "propertyId" = ? AND "item" = ?"#)
                .bind(guild_id)
                .bind(property)
                .bind(item)
                .execute(db)
                .await?;
        return Ok(result.rows_affected() > 0);
    }
    sqlx::query(
        r#"INSERT INTO "CoffreThreshold" ("guildId", "propertyId", "item", "min") VALUES (?, ?, ?, ?)
           ON CONFLICT ("guildId", "propertyId", "item") DO UPDATE SET "min" = excluded."min""#,
    )
    .bind(guild_id)
    .bind(property)
    .bind(item)
    .bind(min)
    .execute(db)
    .await?;
    Ok(true)
}

/// Réglages des alertes d'un serveur (par défaut : désactivées).
pub async fn get_alerts(db: &SqlitePool, guild_id: &str) -> Result<CoffreAlerts> {
    type Row = (Option<String>, Option<String>, Option<i64>, bool);
    let row: Option<Row> = sqlx::query_as(
        r#"SELECT "channelId", "roleId", "bigWithdrawal", "unknownPlayer" FROM "CoffreAlertConfig" WHERE "guildId" = ?"#,
    )
    .bind(guild_id)
    .fetch_optional(db)
    .await?;
    let (channel_id, role_id, big_withdrawal, unknown_player) = row.unwrap_or_default();
    Ok(CoffreAlerts {
        channel_id,
        role_id,
        thresholds: list_thresholds(db, guild_id).await?,
        big_withdrawal,
        unknown_player,
    })
}

/// Remplace tous les réglages des alertes, seuils compris (le tableau de bord
/// du stock n'est pas touché). Refuse une saisie invalide (voir [`CoffreAlerts::validate`]).
pub async fn set_alerts(db: &SqlitePool, guild_id: &str, alerts: &CoffreAlerts) -> Result<()> {
    if let Err(message) = alerts.validate() {
        bail!(message);
    }
    let mut tx = db.begin().await?;
    sqlx::query(
        r#"INSERT INTO "CoffreAlertConfig" ("guildId", "channelId", "roleId", "bigWithdrawal", "unknownPlayer", "updatedAt")
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT ("guildId") DO UPDATE SET "channelId" = excluded."channelId", "roleId" = excluded."roleId",
             "bigWithdrawal" = excluded."bigWithdrawal", "unknownPlayer" = excluded."unknownPlayer", "updatedAt" = excluded."updatedAt""#,
    )
    .bind(guild_id)
    .bind(&alerts.channel_id)
    .bind(&alerts.role_id)
    .bind(alerts.big_withdrawal)
    .bind(alerts.unknown_player)
    .bind(DbTime::now())
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM "CoffreThreshold" WHERE "guildId" = ?"#).bind(guild_id).execute(&mut *tx).await?;
    for threshold in &alerts.thresholds {
        sqlx::query(
            r#"INSERT OR REPLACE INTO "CoffreThreshold" ("guildId", "propertyId", "item", "min") VALUES (?, ?, ?, ?)"#,
        )
        .bind(guild_id)
        .bind(threshold.property_id.as_deref().map(str::trim).unwrap_or_default())
        .bind(threshold.item.trim())
        .bind(threshold.min)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Stock d'un objet passé sous son seuil pendant le lot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LowStock {
    pub property_id: String,
    pub item: String,
    pub before: i64,
    pub after: i64,
    pub min: i64,
}

/// Retrait signalé : gros retrait et/ou personnage inconnu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlaggedWithdrawal {
    pub movement: Movement,
    pub at: DbTime,
    pub big: bool,
    pub unknown: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlertSummary {
    pub low: Vec<LowStock>,
    pub withdrawals: Vec<FlaggedWithdrawal>,
}

impl AlertSummary {
    pub fn is_empty(&self) -> bool {
        self.low.is_empty() && self.withdrawals.is_empty()
    }
}

/// Alertes d'un lot de nouveaux mouvements. Un objet n'est signalé en stock
/// bas que s'il était au moins au seuil avant le lot et passe dessous après :
/// un stock déjà bas ne relance pas d'alerte à chaque retrait.
pub fn evaluate(
    recorded: &[Recorded],
    alerts: &CoffreAlerts,
    thresholds: &Thresholds,
    links: &HashMap<String, String>,
) -> AlertSummary {
    // (propriété, objet) → (stock avant le lot, stock après), dans l'ordre d'apparition.
    let mut spans: Vec<((String, String), i64, i64)> = Vec::new();
    for record in recorded {
        let key = (record.movement.property_id.clone(), record.movement.item.clone());
        match spans.iter_mut().find(|(k, _, _)| *k == key) {
            Some(span) => span.2 = record.stock_after,
            None => spans.push((key, record.stock_before, record.stock_after)),
        }
    }
    let low = spans
        .into_iter()
        .filter_map(|((property_id, item), before, after)| {
            let min = thresholds.min_for(&property_id, &item)?;
            (before >= min && after < min).then_some(LowStock { property_id, item, before, after, min })
        })
        .collect();

    let withdrawals = recorded
        .iter()
        .filter(|r| r.movement.kind == Kind::Withdraw)
        .filter_map(|r| {
            let big = alerts.big_withdrawal.is_some_and(|limit| r.movement.amount >= limit);
            let unknown =
                alerts.unknown_player && r.movement.player_game_id.as_ref().is_none_or(|id| !links.contains_key(id));
            (big || unknown).then(|| FlaggedWithdrawal { movement: r.movement.clone(), at: r.at, big, unknown })
        })
        .collect();
    AlertSummary { low, withdrawals }
}

/// Texte de l'alerte (description d'embed), dans la limite de Discord.
pub fn render(summary: &AlertSummary, names: &HashMap<String, String>) -> String {
    let mut sections = Vec::new();
    if !summary.low.is_empty() {
        let lines: Vec<String> = summary
            .low
            .iter()
            .map(|l| {
                format!(
                    "- **{}** — {} : **{}** (seuil {}, avant {})",
                    item_label(&l.item),
                    property_label(names, &l.property_id),
                    l.after,
                    l.min,
                    l.before
                )
            })
            .collect();
        sections.push(("### 📉 Stock bas".to_owned(), lines));
    }
    if !summary.withdrawals.is_empty() {
        let lines: Vec<String> = summary
            .withdrawals
            .iter()
            .map(|w| {
                let m = &w.movement;
                let player = match &m.player_game_id {
                    Some(id) => format!("{} ({id})", m.player_name),
                    None => m.player_name.clone(),
                };
                let mut tags = Vec::new();
                if w.big {
                    tags.push("gros retrait");
                }
                if w.unknown {
                    tags.push("personnage relié à aucun membre");
                }
                format!(
                    "- `{}` **{}** × {} — {} · {} — *{}*",
                    format_date_label(w.at),
                    item_label(&m.item),
                    m.amount,
                    player,
                    property_label(names, &m.property_id),
                    tags.join(", ")
                )
            })
            .collect();
        sections.push(("### 📤 Retraits à surveiller".to_owned(), lines));
    }
    let count = sections.len().max(1);
    let budget = DESCRIPTION_LIMIT / count - 50;
    sections
        .into_iter()
        .map(|(header, lines)| {
            let body = bounded_lines(&lines, budget.saturating_sub(header.len() + 1), |n| {
                format!("… et {n} autre{}", plural(n))
            });
            format!("{header}\n{body}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Publie l'alerte d'un lot de nouveaux mouvements, si des alertes sont réglées.
pub async fn notify(ctx: &Ctx, guild_id: &str, recorded: &[Recorded]) -> Result<()> {
    if recorded.is_empty() {
        return Ok(());
    }
    let alerts = get_alerts(&ctx.db, guild_id).await?;
    let Some(channel_id) = alerts.channel_id.as_deref().and_then(parse_id) else {
        return Ok(());
    };
    let thresholds = Thresholds::new(&alerts.thresholds);
    let links = if alerts.unknown_player { service::player_links(&ctx.db, guild_id).await? } else { HashMap::new() };
    let summary = evaluate(recorded, &alerts, &thresholds, &links);
    if summary.is_empty() {
        return Ok(());
    }
    let names: HashMap<String, String> = service::properties(&ctx.db, guild_id)
        .await?
        .into_iter()
        .filter_map(|(id, name)| name.map(|name| (id, name)))
        .collect();
    let embed = EmbedBuilder::new()
        .title("🚨 Alerte coffres")
        .color(COLOR)
        .description(render(&summary, &names))
        .footer(footer("Réglages : /config coffre · seuils : /coffre seuils"))
        .build();
    let mut message = Outgoing::embeds(vec![embed]);
    // Seul le rôle configuré est mentionné.
    if let Some(role) = alerts.role_id.as_deref().and_then(parse_id) {
        message.content = Some(format!("<@&{role}>"));
        message.mentions = Mentions::Explicit { users: Vec::new(), roles: vec![role], everyone: false };
    }
    messages::send(ctx, channel_id, &message).await?;
    tracing::info!(
        guild = guild_id,
        low = summary.low.len(),
        withdrawals = summary.withdrawals.len(),
        "alerte coffres"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(property: &str, item: &str, kind: Kind, amount: i64, before: i64, player: Option<&str>) -> Recorded {
        let after = if kind == Kind::Deposit { before + amount } else { before - amount };
        Recorded {
            movement: Movement {
                source: "Coffre propriété".into(),
                property_id: property.into(),
                chest: None,
                item: item.into(),
                amount,
                kind,
                player_name: "Joueur".into(),
                player_game_id: player.map(str::to_owned),
                at: None,
            },
            at: DbTime::now(),
            stock_before: before,
            stock_after: after,
        }
    }

    fn threshold(property: Option<&str>, item: &str, min: i64) -> Threshold {
        Threshold { property_id: property.map(str::to_owned), item: item.into(), min }
    }

    #[test]
    fn specific_threshold_overrides_global() {
        let t = Thresholds::new(&[threshold(None, "kit", 5), threshold(Some("8740"), "kit", 20)]);
        assert_eq!(t.min_for("8740", "kit"), Some(20));
        assert_eq!(t.min_for("1", "kit"), Some(5));
        assert_eq!(t.min_for("1", "pain"), None);
        assert!(t.is_low("8740", "kit", 19));
        assert!(!t.is_low("1", "kit", 5));
    }

    #[test]
    fn alerts_only_when_crossing_below() {
        let alerts = CoffreAlerts { thresholds: vec![threshold(None, "kit", 5)], ..Default::default() };
        let t = Thresholds::new(&alerts.thresholds);
        let links = HashMap::new();
        // 6 → 3 : passage sous le seuil.
        let crossing = [record("1", "kit", Kind::Withdraw, 3, 6, Some("9"))];
        let summary = evaluate(&crossing, &alerts, &t, &links);
        assert_eq!(
            summary.low,
            vec![LowStock { property_id: "1".into(), item: "kit".into(), before: 6, after: 3, min: 5 }]
        );
        // Déjà sous le seuil : pas de nouvelle alerte.
        let already = [record("1", "kit", Kind::Withdraw, 1, 3, Some("9"))];
        assert!(evaluate(&already, &alerts, &t, &links).is_empty());
        // Exactement au seuil après : pas encore bas.
        let at_min = [record("1", "kit", Kind::Withdraw, 1, 6, Some("9"))];
        assert!(evaluate(&at_min, &alerts, &t, &links).is_empty());
        // Sous puis au-dessus dans le même lot : bilan du lot, pas d'alerte.
        let back = [record("1", "kit", Kind::Withdraw, 3, 6, Some("9")), record("1", "kit", Kind::Deposit, 5, 3, None)];
        assert!(evaluate(&back, &alerts, &t, &links).low.is_empty());
        // Deux retraits dans le lot : une seule alerte, du premier stock au dernier.
        let twice =
            [record("1", "kit", Kind::Withdraw, 1, 6, Some("9")), record("1", "kit", Kind::Withdraw, 2, 5, None)];
        let low = evaluate(&twice, &alerts, &t, &links).low;
        assert_eq!((low.len(), low[0].before, low[0].after), (1, 6, 3));
    }

    #[test]
    fn flags_big_and_unknown_withdrawals() {
        let alerts = CoffreAlerts { big_withdrawal: Some(50), unknown_player: true, ..Default::default() };
        let t = Thresholds::default();
        let links: HashMap<String, String> = [("1".to_owned(), "u1".to_owned())].into();
        let batch = [
            record("p", "kit", Kind::Withdraw, 50, 100, Some("1")),
            record("p", "kit", Kind::Withdraw, 1, 50, Some("2")),
            record("p", "kit", Kind::Withdraw, 1, 49, None),
            record("p", "kit", Kind::Deposit, 500, 48, Some("3")),
            record("p", "kit", Kind::Withdraw, 2, 548, Some("1")),
        ];
        let flagged = evaluate(&batch, &alerts, &t, &links).withdrawals;
        assert_eq!(
            flagged.iter().map(|w| (w.big, w.unknown)).collect::<Vec<_>>(),
            [(true, false), (false, true), (false, true)]
        );
        let off = CoffreAlerts::default();
        assert!(evaluate(&batch, &off, &t, &links).is_empty());
        let text = render(&evaluate(&batch, &alerts, &t, &links), &HashMap::new());
        assert!(text.contains("gros retrait") && text.contains("Propriété p"));
        assert!(text.len() <= DESCRIPTION_LIMIT);
    }

    #[test]
    fn validates_settings() {
        let mut alerts = CoffreAlerts { channel_id: Some("123".into()), ..Default::default() };
        assert!(alerts.validate().is_ok());
        alerts.big_withdrawal = Some(0);
        assert!(alerts.validate().is_err());
        alerts.big_withdrawal = None;
        alerts.thresholds = vec![threshold(None, " ", 3)];
        assert!(alerts.validate().is_err());
        alerts.thresholds = vec![threshold(None, "kit", 0)];
        assert!(alerts.validate().is_err());
        alerts.channel_id = Some("abc".into());
        alerts.thresholds.clear();
        assert!(alerts.validate().is_err());
    }

    #[test]
    fn serializes_like_the_typescript_contract() {
        let alerts = CoffreAlerts {
            channel_id: Some("1".into()),
            role_id: None,
            thresholds: vec![threshold(None, "kit", 3)],
            big_withdrawal: Some(10),
            unknown_player: true,
        };
        let json = serde_json::to_value(&alerts).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "channelId": "1", "roleId": null, "bigWithdrawal": 10, "unknownPlayer": true,
                "thresholds": [{ "propertyId": null, "item": "kit", "min": 3 }]
            })
        );
        let back: CoffreAlerts = serde_json::from_value(json).unwrap();
        assert_eq!(back, alerts);
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!("coffre-alert-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        assert_eq!(get_alerts(&db, "g").await.unwrap(), CoffreAlerts::default());
        assert!(set_threshold(&db, "g", None, "kit", 5).await.unwrap());
        assert!(set_threshold(&db, "g", Some("8740"), "kit", 9).await.unwrap());
        let t = thresholds(&db, "g").await.unwrap();
        assert_eq!((t.min_for("8740", "kit"), t.min_for("1", "kit")), (Some(9), Some(5)));
        assert!(set_threshold(&db, "g", Some("8740"), "kit", 0).await.unwrap());
        assert!(!set_threshold(&db, "g", Some("8740"), "kit", 0).await.unwrap());

        let alerts = CoffreAlerts {
            channel_id: Some("10".into()),
            role_id: Some("20".into()),
            thresholds: vec![threshold(Some("1"), "pain", 3)],
            big_withdrawal: Some(100),
            unknown_player: true,
        };
        set_alerts(&db, "g", &alerts).await.unwrap();
        assert_eq!(get_alerts(&db, "g").await.unwrap(), alerts);
        assert!(set_alerts(&db, "g", &CoffreAlerts { big_withdrawal: Some(-1), ..Default::default() }).await.is_err());
        assert!(get_alerts(&db, "autre").await.unwrap().thresholds.is_empty());

        // Stock avant / après rapporté par l'enregistrement des mouvements.
        service::add_channel(&db, "g", "c").await.unwrap();
        let batch: Vec<service::NewMovement> = [("m1:0", Kind::Deposit, 8), ("m2:0", Kind::Withdraw, 6)]
            .into_iter()
            .map(|(id, kind, amount)| service::NewMovement {
                id: id.into(),
                movement: record("1", "pain", kind, amount, 0, Some("7")).movement,
                at: DbTime::now(),
            })
            .collect();
        let recorded = service::insert_all(&db, "g", "c", &batch, None).await.unwrap();
        assert_eq!(recorded.iter().map(|r| (r.stock_before, r.stock_after)).collect::<Vec<_>>(), [(0, 8), (8, 2)]);
        let summary = evaluate(&recorded, &alerts, &Thresholds::new(&alerts.thresholds), &HashMap::new());
        // 0 → 2 sur le lot : le stock n'a jamais été au seuil, pas d'alerte de stock bas.
        assert!(summary.low.is_empty());
        assert_eq!(summary.withdrawals.len(), 1);
        assert!(service::insert_all(&db, "g", "c", &batch, None).await.unwrap().is_empty());

        db.close().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
