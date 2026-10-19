//! Coffres d'un serveur : stock par propriété, activité par joueur, mouvements
//! filtrables et export CSV. Accès : `coffre.voir`.

use super::transactions::{number_cell, text_cell};
use crate::{
    context::Ctx,
    modules::coffres::{
        commands::property_label,
        parser::Kind,
        service::{self, Filter, MovementRow},
    },
    util::time::{DbTime, format_date_label},
};
use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::Duration;
use serde::Deserialize;
use std::collections::HashMap;
use twilight_model::id::{Id, marker::GuildMarker};

pub(crate) const PAGE_SIZE: i64 = 50;
const CSV_LIMIT: i64 = 20_000;
/// Période affichée sans choix explicite.
const DEFAULT_DAYS: i64 = 30;

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CoffreQuery {
    /// Une seule propriété.
    propriete: Option<String>,
    /// Un seul objet.
    objet: Option<String>,
    /// Identifiant de jeu d'un personnage.
    joueur: Option<String>,
    /// `deposit` ou `withdraw`.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Nombre de jours (30 par défaut), ou `tout`.
    jours: Option<String>,
    /// Page (1 = première).
    page: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct Parsed {
    pub(crate) property: Option<String>,
    pub(crate) item: Option<String>,
    pub(crate) player: Option<String>,
    pub(crate) kind: Option<Kind>,
    pub(crate) days: Option<i64>,
    pub(crate) page: i64,
}

fn clean(value: Option<&String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty() && v.len() <= 100)
}

impl CoffreQuery {
    pub(crate) fn parse(&self) -> Parsed {
        Parsed {
            property: clean(self.propriete.as_ref()),
            item: clean(self.objet.as_ref()),
            player: clean(self.joueur.as_ref()).filter(|p| p.bytes().all(|b| b.is_ascii_digit())),
            kind: self.kind.as_deref().and_then(Kind::parse),
            // 30 jours par défaut : l'historique complet (« tout ») est à demander.
            days: match self.jours.as_deref() {
                None => Some(DEFAULT_DAYS),
                Some("tout") => None,
                Some(d) => Some(d.parse().ok().filter(|d| (1..=3650).contains(d)).unwrap_or(DEFAULT_DAYS)),
            },
            page: self.page.as_deref().and_then(|p| p.parse().ok()).unwrap_or(1).max(1),
        }
    }
}

impl Parsed {
    pub(crate) fn filter(&self) -> Filter {
        Filter {
            property_id: self.property.clone(),
            item: self.item.clone(),
            player_game_ids: self.player.clone().map(|p| vec![p]),
            kind: self.kind,
            since: self.days.map(|d| DbTime(DbTime::now().0 - Duration::days(d))),
        }
    }
}

fn build_csv(rows: &[MovementRow], names: &HashMap<String, String>) -> String {
    let mut csv = String::from("\u{feff}Date;Propriété;Coffre;Objet;Type;Quantité;Joueur;Identifiant jeu\r\n");
    for row in rows {
        let line = [
            format_date_label(row.at),
            text_cell(&property_label(names, &row.property_id)),
            row.chest.map(|c| c.to_string()).unwrap_or_default(),
            text_cell(&row.item),
            row.kind().label().to_owned(),
            number_cell(row.amount as f64),
            text_cell(&row.player_name),
            row.player_game_id.clone().unwrap_or_default(),
        ];
        csv.push_str(&line.join(";"));
        csv.push_str("\r\n");
    }
    csv
}

/// Export CSV des mouvements filtrés (page et API).
pub(crate) async fn csv_response(ctx: &Ctx, guild_id: Id<GuildMarker>, query: &CoffreQuery) -> Response {
    let guild = guild_id.to_string();
    let data = async {
        let rows = service::movements(&ctx.db, &guild, &query.parse().filter(), CSV_LIMIT, 0).await?;
        let names: HashMap<String, String> = service::properties(&ctx.db, &guild)
            .await?
            .into_iter()
            .filter_map(|(id, name)| name.map(|n| (id, n)))
            .collect();
        anyhow::Ok(build_csv(&rows, &names))
    }
    .await;
    match data {
        Ok(csv) => {
            let filename = format!("coffres-{guild}-{}.csv", chrono::Utc::now().format("%Y-%m-%d"));
            (
                [
                    (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
                    (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
                    (header::CACHE_CONTROL, "no-store".to_owned()),
                ],
                csv,
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(?error, "export des coffres");
            (StatusCode::INTERNAL_SERVER_ERROR, "Export indisponible").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_filters() {
        let query = CoffreQuery {
            propriete: Some("8740".into()),
            objet: Some("repair_kit".into()),
            joueur: Some("340772".into()),
            kind: Some("withdraw".into()),
            jours: Some("7".into()),
            page: Some("3".into()),
        };
        let parsed = query.parse();
        assert_eq!(
            (parsed.property.as_deref(), parsed.item.as_deref(), parsed.player.as_deref(), parsed.days, parsed.page),
            (Some("8740"), Some("repair_kit"), Some("340772"), Some(7), 3)
        );
        let bad = CoffreQuery { joueur: Some("1 OR 1".into()), kind: Some("x".into()), ..Default::default() }.parse();
        assert_eq!((bad.player, bad.kind, bad.page), (None, None, 1));
    }
}
