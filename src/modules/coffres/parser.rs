//! Lecture des encadrés publiés par le webhook du jeu :
//!
//! ```text
//! Coffre propriété
//! Initiator: Kyle Ian Bappy Duggan (340772)   ID: ["8740",1]   Item: repair_kit
//! Amount: 7                                    Type: Withdraw
//! 19:20:13 - 18/10/2026
//! ```

use crate::util::time::{DbTime, parse_event_date};
use serde_json::Value;
use std::sync::LazyLock;
use twilight_model::channel::message::Embed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Deposit,
    Withdraw,
    /// Correction d'inventaire saisie par un gradé (quantité signée), jamais lue dans le jeu.
    Adjust,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deposit => "DEPOSIT",
            Self::Withdraw => "WITHDRAW",
            Self::Adjust => "ADJUST",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "deposit" | "dépôt" | "depot" => Some(Self::Deposit),
            "withdraw" | "retrait" => Some(Self::Withdraw),
            "adjust" | "correction" => Some(Self::Adjust),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Deposit => "Dépôt",
            Self::Withdraw => "Retrait",
            Self::Adjust => "Correction",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Movement {
    pub source: String,
    pub property_id: String,
    pub chest: Option<i64>,
    pub item: String,
    pub amount: i64,
    pub kind: Kind,
    pub player_name: String,
    pub player_game_id: Option<String>,
    /// Heure indiquée dans l'encadré (heure de Paris), sinon `None`.
    pub at: Option<DbTime>,
}

static INITIATOR: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^(.*?)\s*\((\d+)\)\s*$").expect("regex valide"));
static STAMP: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(\d{1,2}:\d{2})(?::\d{2})?\s*-\s*(\d{1,2}/\d{1,2}/\d{4})").expect("regex valide")
});

fn field<'a>(embed: &'a Embed, names: &[&str]) -> Option<&'a str> {
    embed.fields.iter().find(|f| names.iter().any(|n| f.name.trim().eq_ignore_ascii_case(n))).map(|f| f.value.trim())
}

/// `["8740",1]` → (« 8740 », 1). Accepte aussi une valeur seule.
fn property(value: &str) -> Option<(String, Option<i64>)> {
    let text = |v: &Value| match v {
        Value::String(s) => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    match serde_json::from_str::<Value>(value) {
        Ok(Value::Array(parts)) => {
            let id = parts.first().and_then(text).filter(|s| !s.is_empty())?;
            let chest = parts.get(1).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));
            Some((id, chest))
        }
        Ok(other) => text(&other).filter(|s| !s.is_empty()).map(|id| (id, None)),
        Err(_) => Some((value.trim().trim_matches('"').to_owned(), None)).filter(|(id, _)| !id.is_empty()),
    }
}

/// Seconde comprise : l'heure de l'encadré est à la seconde, elle sert à trier.
fn stamp(text: &str) -> Option<DbTime> {
    let caps = STAMP.captures(text)?;
    let time = text[caps.get(1)?.start()..caps.get(0)?.end()].split('-').next()?.trim();
    let (hm, seconds) = match time.rsplit_once(':') {
        Some((hm, s)) if hm.contains(':') => (hm, s.parse::<i64>().unwrap_or(0)),
        _ => (time, 0),
    };
    parse_event_date(&caps[2], hm).map(|d| DbTime(d.0 + chrono::Duration::seconds(seconds)))
}

/// Quantité maximale acceptée : au-delà, l'encadré est ignoré (protège les totaux).
pub const MAX_AMOUNT: i64 = 1_000_000;

/// Un mouvement de coffre, si l'encadré en est un.
pub fn parse(embed: &Embed) -> Option<Movement> {
    let source = embed.title.as_deref().map(str::trim).filter(|t| t.to_lowercase().starts_with("coffre"))?;
    let (property_id, chest) = property(field(embed, &["ID", "Propriété", "Property"])?)?;
    let item = field(embed, &["Item", "Objet"])?.to_owned();
    let amount = field(embed, &["Amount", "Quantité", "Quantity"])?
        .replace([' ', '\u{202f}', '\u{a0}'], "")
        .parse::<i64>()
        .ok()?;
    // Le jeu ne publie que des dépôts et des retraits : une « correction » imitée est ignorée.
    let kind = Kind::parse(field(embed, &["Type"])?).filter(|k| *k != Kind::Adjust)?;
    let initiator = field(embed, &["Initiator", "Joueur", "Player"]).unwrap_or_default();
    let (player_name, player_game_id) = match INITIATOR.captures(initiator) {
        Some(caps) => (caps[1].trim().to_owned(), Some(caps[2].to_owned())),
        None => (initiator.to_owned(), None),
    };
    let at = [embed.footer.as_ref().map(|f| f.text.as_str()), embed.description.as_deref()]
        .into_iter()
        .flatten()
        .find_map(stamp);
    (amount > 0 && amount <= MAX_AMOUNT && !item.is_empty()).then(|| Movement {
        source: source.to_owned(),
        property_id,
        chest,
        item,
        amount,
        kind,
        player_name,
        player_game_id,
        at,
    })
}

/// `repair_kit` → « Repair kit ».
pub fn item_label(item: &str) -> String {
    let spaced = item.replace(['_', '-'], " ");
    let mut chars = spaced.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => item.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twilight_model::channel::message::embed::{EmbedField, EmbedFooter};

    fn embed(kind: &str, amount: &str, footer: &str) -> Embed {
        let field = |name: &str, value: &str| EmbedField { inline: true, name: name.into(), value: value.into() };
        Embed {
            author: None,
            color: Some(0xe74c3c),
            description: None,
            fields: vec![
                field("Initiator", "Kyle Ian Bappy Duggan (340772)"),
                field("ID", r#"["8740",1]"#),
                field("Item", "repair_kit"),
                field("Amount", amount),
                field("Type", kind),
            ],
            footer: Some(EmbedFooter { icon_url: None, proxy_icon_url: None, text: footer.into() }),
            image: None,
            kind: "rich".into(),
            provider: None,
            thumbnail: None,
            timestamp: None,
            title: Some("Coffre propriété".into()),
            url: None,
            video: None,
        }
    }

    #[test]
    fn parses_webhook_embeds() {
        let movement = parse(&embed("Withdraw", "7", "19:20:13 - 18/10/2026")).unwrap();
        assert_eq!(movement.property_id, "8740");
        assert_eq!(movement.chest, Some(1));
        assert_eq!(movement.item, "repair_kit");
        assert_eq!((movement.amount, movement.kind), (7, Kind::Withdraw));
        assert_eq!(
            (movement.player_name.as_str(), movement.player_game_id.as_deref()),
            ("Kyle Ian Bappy Duggan", Some("340772"))
        );
        // 19:20:13 à Paris (heure d'été) = 17:20:13 UTC.
        assert_eq!(movement.at.unwrap().0.to_rfc3339(), "2026-10-18T17:20:13+00:00");

        let deposit = parse(&embed("Deposit", "8", "")).unwrap();
        assert_eq!((deposit.kind, deposit.amount, deposit.at), (Kind::Deposit, 8, None));
    }

    #[test]
    fn ignores_other_embeds() {
        let mut other = embed("Deposit", "1", "");
        other.title = Some("Facture".into());
        assert!(parse(&other).is_none());
        assert!(parse(&embed("Steal", "1", "")).is_none());
        assert!(parse(&embed("Deposit", "abc", "")).is_none());
        // Quantités absurdes (encadré imité) : ignorées, les totaux restent justes.
        assert!(parse(&embed("Deposit", "9223372036854775807", "")).is_none());
        assert!(parse(&embed("Deposit", &(MAX_AMOUNT + 1).to_string(), "")).is_none());
        assert!(parse(&embed("Deposit", "0", "")).is_none());
        // Une « correction » ne vient jamais du jeu : un encadré qui en imite une est ignoré.
        assert!(parse(&embed("Correction", "5", "")).is_none());
    }

    #[test]
    fn reads_property_ids() {
        assert_eq!(property(r#"["8740",1]"#), Some(("8740".into(), Some(1))));
        assert_eq!(property(r#"[12,"3"]"#), Some(("12".into(), Some(3))));
        assert_eq!(property("8740"), Some(("8740".into(), None)));
        assert_eq!(item_label("repair_kit"), "Repair kit");
    }
}
