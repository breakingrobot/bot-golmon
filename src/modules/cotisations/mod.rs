//! Cotisations : ce que les membres rapportent au groupe, en argent propre
//! et/ou sale. Chacun déclare ses cotisations ; les gradés et leads consultent
//! les statistiques, sur Discord et sur le site (niveaux : `/config permission`).

pub mod commands;
pub mod cron;
pub mod dashboard;
pub mod objective;
pub mod service;

/// Montant lisible : « 12 500 $ ».
pub fn money(value: i64) -> String {
    format!("{}\u{202f}$", crate::util::format::format_price(value as f64))
}
