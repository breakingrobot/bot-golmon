pub mod phones;
pub mod presences;
pub mod recrues;
pub mod transactions;
pub mod vehicles;

/// Motif `LIKE` « contient », avec échappement des jokers saisis par l'utilisateur.
pub fn like_contains(query: &str) -> String {
    let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}
