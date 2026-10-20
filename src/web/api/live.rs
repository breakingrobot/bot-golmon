//! Mise à jour en direct : flux SSE des changements d'un serveur.
//!
//! Chaque message (`event: change`) ne porte que le **sujet** (`coffres`,
//! `cotisations`…) : l'interface recharge alors ses requêtes de ce sujet par
//! l'API, avec les droits de la personne. Aucune donnée ne passe par le flux.
//! `*` : des changements ont été perdus (abonné trop lent), tout recharger.

use super::{ApiError, ApiResult, Session};
use crate::{
    context::Context,
    events::{ConnectionGuard, GuildEvent},
};
use axum::{
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{
    StreamExt as _,
    stream::{self, Stream},
};
use std::{convert::Infallible, time::Duration};
use tokio::sync::broadcast::{Receiver, error::RecvError};

/// Commentaire envoyé régulièrement : garde la connexion ouverte à travers les proxys.
const KEEP_ALIVE: Duration = Duration::from_secs(25);
/// Sujet envoyé quand des changements ont été perdus.
pub const RESYNC: &str = "*";

/// Sujet à envoyer pour un événement reçu : seulement ceux du serveur suivi.
pub fn topic_for(event: &GuildEvent, guild_id: &str) -> Option<&'static str> {
    event.is_for(guild_id).then(|| event.topic.as_str())
}

/// Sujets d'un serveur, lus sur le bus jusqu'à sa fermeture. Le jeton de
/// connexion vit avec le flux : la place est rendue à la déconnexion.
pub fn topics(
    receiver: Receiver<GuildEvent>,
    guild_id: String,
    guard: ConnectionGuard,
) -> impl Stream<Item = &'static str> {
    stream::unfold((receiver, guild_id, guard), |(mut receiver, guild_id, guard)| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if let Some(topic) = topic_for(&event, &guild_id) {
                        return Some((topic, (receiver, guild_id, guard)));
                    }
                }
                Err(RecvError::Lagged(_)) => return Some((RESYNC, (receiver, guild_id, guard))),
                Err(RecvError::Closed) => return None,
            }
        }
    })
}

/// Changements du serveur en direct (membre du serveur).
#[utoipa::path(
    get, path = "/api/guilds/{guild}/events", tag = "stats", operation_id = "getGuildEvents",
    params(("guild" = String, Path, description = "Identifiant du serveur")),
    responses(
        (status = 200, description = "Flux `text/event-stream` : `event: change`, `data: <sujet>`", content_type = "text/event-stream", body = String),
        (status = 401, description = "Pas connecté", body = super::ErrorBody),
        (status = 403, description = "Pas membre du serveur", body = super::ErrorBody),
        (status = 429, description = "Trop de connexions ouvertes", body = super::ErrorBody),
    )
)]
pub async fn events(State(ctx): State<Context>, session: Session, Path(guild): Path<String>) -> ApiResult {
    let guild_id = session.member_of(&guild)?;
    let Some(guard) = ctx.events.connect(session.viewer.id.get()) else {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Trop de pages ouvertes en direct : ferme un onglet du site.",
        ));
    };
    let stream = topics(ctx.events.subscribe(), guild_id.to_string(), guard)
        .map(|topic| Ok::<_, Infallible>(Event::default().event("change").data(topic)));
    let mut response = Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE)).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    // Nginx et consorts : pas de mise en tampon, chaque message part tout de suite.
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventBus, Topic};

    #[test]
    fn only_topics_of_the_followed_guild_pass() {
        let event = GuildEvent { guild_id: "10".into(), topic: Topic::Coffres };
        assert_eq!(topic_for(&event, "10"), Some("coffres"));
        assert_eq!(topic_for(&event, "11"), None);
    }

    #[tokio::test]
    async fn stream_filters_by_guild_and_frees_its_slot() {
        let bus = EventBus::default();
        let guard = bus.connect(42).unwrap();
        let stream = topics(bus.subscribe(), "10".into(), guard);
        bus.publish("11", Topic::Coffres);
        bus.publish("10", Topic::Cotisations);
        bus.publish(10_u64, Topic::Journal);
        let received: Vec<&str> = stream.take(2).collect().await;
        assert_eq!(received, ["cotisations", "journal"]);
        assert_eq!(bus.connections_of(42), 0, "flux terminé : place rendue");
    }
}
