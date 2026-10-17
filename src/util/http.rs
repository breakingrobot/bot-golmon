//! Client HTTP sortant unique (API GLife, pièces jointes, connexion Discord…).
//!
//! Chaque client reqwest garde son pool de connexions et sa configuration TLS
//! (magasin de certificats racines compris) : un seul client partagé évite de
//! les dupliquer. Les délais sont fixés requête par requête.

use std::sync::LazyLock;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(concat!("bot-discord/", env!("CARGO_PKG_VERSION")))
        // Connexions inactives libérées vite : le bot appelle ces API rarement.
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .build()
        .expect("client HTTP")
});

pub fn client() -> reqwest::Client {
    CLIENT.clone()
}
