//! Identifiants et `custom_id` des composants.

use rand::Rng;

/// Identifiant façon cuid (25 caractères, minuscules et chiffres), même allure
/// que ceux générés par Prisma dans la version précédente.
pub fn new_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    let mut id = String::with_capacity(25);
    id.push('c');
    for _ in 0..24 {
        id.push(ALPHABET[rng.random_range(0..ALPHABET.len())] as char);
    }
    id
}

/// `namespace:action:id[:payload]`. Le contenu d'un `custom_id` est fourni par
/// le client : il est toujours revalidé à la lecture.
pub fn encode_custom_id(namespace: &str, action: &str, id: &str, payload: Option<&str>) -> String {
    match payload {
        Some(payload) => format!("{namespace}:{action}:{id}:{payload}"),
        None => format!("{namespace}:{action}:{id}"),
    }
}

pub struct CustomId<'a> {
    pub namespace: &'a str,
    pub action: &'a str,
    pub id: &'a str,
    pub payload: Option<&'a str>,
}

pub fn decode_custom_id(raw: &str) -> Option<CustomId<'_>> {
    let mut parts = raw.split(':');
    let namespace = parts.next().filter(|s| !s.is_empty())?;
    let action = parts.next().filter(|s| !s.is_empty())?;
    let id = parts.next().filter(|s| !s.is_empty())?;
    Some(CustomId { namespace, action, id, payload: parts.next() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_custom_ids() {
        let raw = encode_custom_id("transaction", "status", "abc", Some("LIVRE_PAYE"));
        let parsed = decode_custom_id(&raw).unwrap();
        assert_eq!(
            (parsed.namespace, parsed.action, parsed.id, parsed.payload),
            ("transaction", "status", "abc", Some("LIVRE_PAYE"))
        );
        assert!(decode_custom_id("a::b").is_none());
    }

    #[test]
    fn ids_look_like_cuids() {
        let id = new_id();
        assert_eq!(id.len(), 25);
        assert!(id.starts_with('c'));
    }
}
