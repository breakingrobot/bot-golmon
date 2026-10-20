//! Inventaires collés en attente de validation (« Valider » / « Annuler »).
//! Gardés en mémoire 15 minutes et réservés à leur auteur : rien n'est écrit
//! avant « Valider ». Un redémarrage du bot les oublie (il suffit de recoller).

use rand::Rng as _;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

/// Durée de vie d'une saisie en attente.
pub const TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub guild_id: String,
    pub user_id: String,
    pub property_id: String,
    pub counted: Vec<(String, i64)>,
}

struct Entry {
    pending: Pending,
    created: Instant,
}

static STORE: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Default::default);

/// Clé aléatoire du `custom_id` des boutons : impossible à deviner.
fn new_key() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..16).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}

/// Ce que donne une clé au moment de valider ou d'annuler.
#[derive(Debug, PartialEq, Eq)]
pub enum Taken {
    Found(Pending),
    /// Saisie d'une autre personne : elle reste en attente.
    NotYours,
    /// Inconnue, expirée ou déjà traitée.
    Missing,
}

fn put_at(store: &mut HashMap<String, Entry>, pending: Pending, now: Instant) -> String {
    store.retain(|_, entry| now.duration_since(entry.created) < TTL);
    let key = new_key();
    store.insert(key.clone(), Entry { pending, created: now });
    key
}

fn take_at(store: &mut HashMap<String, Entry>, key: &str, user_id: &str, now: Instant) -> Taken {
    match store.get(key) {
        None => Taken::Missing,
        Some(entry) if now.duration_since(entry.created) >= TTL => {
            store.remove(key);
            Taken::Missing
        }
        Some(entry) if entry.pending.user_id != user_id => Taken::NotYours,
        Some(_) => store.remove(key).map_or(Taken::Missing, |entry| Taken::Found(entry.pending)),
    }
}

/// Met une saisie en attente ; renvoie sa clé.
pub fn put(pending: Pending) -> String {
    put_at(&mut STORE.lock().expect("verrou des inventaires"), pending, Instant::now())
}

/// Retire une saisie pour la valider ou l'annuler (seulement par son auteur).
pub fn take(key: &str, user_id: &str) -> Taken {
    take_at(&mut STORE.lock().expect("verrou des inventaires"), key, user_id, Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(user: &str) -> Pending {
        Pending {
            guild_id: "g".into(),
            user_id: user.into(),
            property_id: "8740".into(),
            counted: vec![("repair_kit".into(), 42)],
        }
    }

    #[test]
    fn reserved_to_its_author_once() {
        let mut store = HashMap::new();
        let now = Instant::now();
        let key = put_at(&mut store, pending("1"), now);
        assert_eq!(key.len(), 16);
        assert_eq!(take_at(&mut store, &key, "2", now), Taken::NotYours);
        assert_eq!(take_at(&mut store, &key, "1", now), Taken::Found(pending("1")));
        assert_eq!(take_at(&mut store, &key, "1", now), Taken::Missing, "déjà traitée");
    }

    #[test]
    fn expires_after_ttl() {
        let mut store = HashMap::new();
        let now = Instant::now();
        let key = put_at(&mut store, pending("1"), now);
        assert_eq!(take_at(&mut store, &key, "1", now + TTL), Taken::Missing);
        // Les saisies expirées sont purgées à l'ajout suivant.
        put_at(&mut store, pending("1"), now);
        put_at(&mut store, pending("1"), now + TTL + Duration::from_secs(1));
        assert_eq!(store.len(), 1);
    }
}
