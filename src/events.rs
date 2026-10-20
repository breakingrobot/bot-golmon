//! Bus des changements d'un serveur, pour la mise à jour en direct du site
//! (`GET /api/guilds/{guild}/events`, flux SSE).
//!
//! Les modules signalent un **sujet** (« les coffres de tel serveur ont
//! changé ») aux endroits où ils rafraîchissent déjà leurs messages Discord ;
//! le site recharge alors les données concernées par l'API habituelle, avec
//! les droits de la personne. Le bus ne transporte jamais de données.

use std::{
    collections::HashMap,
    fmt::Display,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

/// Événements gardés pour un abonné lent avant qu'il ne décroche (il recharge alors tout).
const CAPACITY: usize = 256;

/// Connexions simultanées par personne (un onglet = une connexion ; les
/// navigateurs n'en ouvrent que six par site en HTTP/1.1).
pub const MAX_CONNECTIONS_PER_USER: usize = 4;

/// Ce qui a changé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    Coffres,
    Cotisations,
    Transactions,
    Contacts,
    Presences,
    Absences,
    Recrues,
    Journal,
    Config,
}

impl Topic {
    #[cfg(test)]
    pub const ALL: [Self; 9] = [
        Self::Coffres,
        Self::Cotisations,
        Self::Transactions,
        Self::Contacts,
        Self::Presences,
        Self::Absences,
        Self::Recrues,
        Self::Journal,
        Self::Config,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Coffres => "coffres",
            Self::Cotisations => "cotisations",
            Self::Transactions => "transactions",
            Self::Contacts => "contacts",
            Self::Presences => "presences",
            Self::Absences => "absences",
            Self::Recrues => "recrues",
            Self::Journal => "journal",
            Self::Config => "config",
        }
    }
}

/// Changement sur un serveur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildEvent {
    pub guild_id: String,
    pub topic: Topic,
}

impl GuildEvent {
    /// Concerne ce serveur ?
    pub fn is_for(&self, guild_id: &str) -> bool {
        self.guild_id == guild_id
    }
}

/// Émetteur partagé (dans `Ctx`) et compte des connexions ouvertes.
pub struct EventBus {
    sender: broadcast::Sender<GuildEvent>,
    connections: Arc<Mutex<HashMap<u64, usize>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self { sender: broadcast::channel(CAPACITY).0, connections: Arc::default() }
    }
}

impl EventBus {
    /// Signale un changement. Sans abonné, l'événement est simplement perdu.
    pub fn publish(&self, guild_id: impl Display, topic: Topic) {
        let _ = self.sender.send(GuildEvent { guild_id: guild_id.to_string(), topic });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GuildEvent> {
        self.sender.subscribe()
    }

    /// Réserve une connexion pour `user` ; `None` au-delà de la limite.
    /// La place est rendue quand le jeton est abandonné (fin du flux).
    pub fn connect(&self, user: u64) -> Option<ConnectionGuard> {
        let mut connections = self.connections.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = connections.entry(user).or_default();
        if *count >= MAX_CONNECTIONS_PER_USER {
            return None;
        }
        *count += 1;
        Some(ConnectionGuard { connections: Arc::clone(&self.connections), user })
    }

    #[cfg(test)]
    pub fn connections_of(&self, user: u64) -> usize {
        self.connections.lock().unwrap().get(&user).copied().unwrap_or(0)
    }
}

/// Connexion ouverte : libère sa place à l'abandon.
pub struct ConnectionGuard {
    connections: Arc<Mutex<HashMap<u64, usize>>>,
    user: u64,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let mut connections = self.connections.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(count) = connections.get_mut(&self.user) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&self.user);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribers_receive_topics_of_every_guild() {
        let bus = EventBus::default();
        // Sans abonné : rien ne casse.
        bus.publish("1", Topic::Coffres);
        let mut receiver = bus.subscribe();
        bus.publish(1_u64, Topic::Cotisations);
        bus.publish("2", Topic::Journal);
        let first = receiver.recv().await.unwrap();
        assert_eq!(first, GuildEvent { guild_id: "1".into(), topic: Topic::Cotisations });
        assert!(first.is_for("1") && !first.is_for("2"));
        assert_eq!(receiver.recv().await.unwrap().topic, Topic::Journal);
    }

    #[test]
    fn connections_are_bounded_per_user() {
        let bus = EventBus::default();
        let guards: Vec<_> = (0..MAX_CONNECTIONS_PER_USER).map(|_| bus.connect(7).unwrap()).collect();
        assert!(bus.connect(7).is_none(), "limite atteinte");
        assert!(bus.connect(8).is_some(), "une autre personne n'est pas concernée");
        drop(guards);
        assert_eq!(bus.connections_of(7), 0);
        assert!(bus.connect(7).is_some());
    }

    #[test]
    fn topics_have_stable_names() {
        let names: Vec<&str> = Topic::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            names,
            [
                "coffres",
                "cotisations",
                "transactions",
                "contacts",
                "presences",
                "absences",
                "recrues",
                "journal",
                "config"
            ]
        );
    }
}
