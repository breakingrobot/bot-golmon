//! Consultation de l'API publique GLife (`GET /roleplay/vehicles?plate=…`).
//!
//! Résultats en cache 5 minutes ; les appels concurrents sur une même plaque
//! partagent une seule requête ; timeout de 8 s pour ne jamais bloquer une
//! interaction Discord.

use futures_util::future::{BoxFuture, FutureExt as _, Shared};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

const API_URL: &str = "https://api.glife.fr/roleplay/vehicles";
const TIMEOUT: Duration = Duration::from_secs(8);
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
/// Au-delà, on balaie les entrées périmées : sans quoi le cache ne rétrécit jamais.
const CACHE_SWEEP_THRESHOLD: usize = 500;

#[derive(Clone, Debug)]
pub struct Party {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct ApiVehicle {
    pub plate: String,
    /// Nom lisible, ex : « Kamacho ».
    pub name: Option<String>,
    /// Code interne du modèle, ex : « kamacho ».
    pub model: Option<String>,
    pub illegal: bool,
    /// Propriétaire en jeu : joueur ou organisation, distinct du propriétaire Discord.
    pub owner: Option<Party>,
    pub buyer: Option<Party>,
}

#[derive(Clone, Debug)]
pub enum Lookup {
    Found(ApiVehicle),
    NotFound,
    Error(String),
}

type Pending = Shared<BoxFuture<'static, Lookup>>;

pub struct VehicleApi {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, (Lookup, Instant)>>,
    in_flight: Mutex<HashMap<String, Pending>>,
}

/// Majuscules, sans espaces ni tirets : évite les doublons et les recherches ratées.
pub fn normalize_plate(plate: &str) -> String {
    plate.trim().to_uppercase().chars().filter(|c| !c.is_whitespace() && *c != '-').collect()
}

/// Les identifiants ne sont pas typés uniformément par l'API (texte ou nombre).
fn optional_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_party(value: &Value) -> Option<Party> {
    optional_string(value.get("name")?).map(|name| Party { name })
}

fn parse_vehicle(value: &Value) -> Option<ApiVehicle> {
    let plate = optional_string(value.get("plate")?)?;
    let field = |key: &str| value.get(key).and_then(optional_string);
    Some(ApiVehicle {
        plate,
        name: field("name"),
        model: field("model"),
        // `illegal` est un entier 0/1 côté API, pas un booléen.
        illegal: matches!(value.get("illegal"), Some(Value::Bool(true)))
            || matches!(value.get("illegal").and_then(optional_string).as_deref(), Some("1")),
        owner: value.get("owner").and_then(parse_party),
        buyer: value.get("buyer").and_then(parse_party),
    })
}

impl VehicleApi {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("bot-golmon/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("client HTTP");
        Self { client, cache: Mutex::new(HashMap::new()), in_flight: Mutex::new(HashMap::new()) }
    }

    async fn request(client: reqwest::Client, plate: String) -> Lookup {
        let response =
            match client.get(API_URL).query(&[("plate", &plate)]).header("accept", "application/json").send().await {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!(%plate, %error, "consultation API échouée");
                    return Lookup::Error(if error.is_timeout() {
                        "délai dépassé".to_owned()
                    } else {
                        error.to_string()
                    });
                }
            };
        // 404 = plaque inconnue, ce n'est pas une panne.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Lookup::NotFound;
        }
        if !response.status().is_success() {
            return Lookup::Error(format!("HTTP {}", response.status().as_u16()));
        }
        match response.json::<Value>().await {
            // L'API renvoie un tableau : un tableau vide vaut « non trouvé ».
            Ok(payload) => {
                let first = match &payload {
                    Value::Array(items) => items.first().cloned().unwrap_or(Value::Null),
                    other => other.clone(),
                };
                parse_vehicle(&first).map_or(Lookup::NotFound, Lookup::Found)
            }
            Err(error) => Lookup::Error(error.to_string()),
        }
    }

    pub async fn lookup(&self, raw_plate: &str) -> Lookup {
        let plate = normalize_plate(raw_plate);
        if plate.is_empty() {
            return Lookup::Error("plaque vide".to_owned());
        }

        if let Some((result, expires)) = self.cache.lock().expect("cache").get(&plate)
            && *expires > Instant::now()
        {
            return result.clone();
        }

        let pending = {
            let mut in_flight = self.in_flight.lock().expect("in_flight");
            in_flight
                .entry(plate.clone())
                .or_insert_with(|| Self::request(self.client.clone(), plate.clone()).boxed().shared())
                .clone()
        };
        let result = pending.await;
        self.in_flight.lock().expect("in_flight").remove(&plate);

        // Les erreurs ne sont pas mises en cache : la prochaine tentative doit réessayer.
        if !matches!(result, Lookup::Error(_)) {
            let now = Instant::now();
            let mut cache = self.cache.lock().expect("cache");
            if cache.len() >= CACHE_SWEEP_THRESHOLD {
                cache.retain(|_, (_, expires)| *expires > now);
            }
            cache.insert(plate, (result.clone(), now + CACHE_TTL));
        }
        result
    }
}

impl Default for VehicleApi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_plates() {
        assert_eq!(normalize_plate(" d6cs 3p0g "), "D6CS3P0G");
        assert_eq!(normalize_plate("D6-CS-3P0G"), "D6CS3P0G");
    }

    #[test]
    fn parses_api_payload() {
        let payload = serde_json::json!({
            "id": 12, "plate": "D6CS3P0G", "name": "Kamacho", "model": "kamacho", "illegal": 0,
            "owner": { "id": "abc", "name": "Los Santos Customs", "type": 2 },
            "buyer": { "id": 42, "name": "John Doe" }
        });
        let vehicle = parse_vehicle(&payload).unwrap();
        assert!(!vehicle.illegal);
        assert_eq!(vehicle.owner.unwrap().name, "Los Santos Customs");
        assert_eq!(vehicle.buyer.unwrap().name, "John Doe");
        let illegal = parse_vehicle(&serde_json::json!({ "plate": "X", "illegal": 1 })).unwrap();
        assert!(illegal.illegal);
    }
}
