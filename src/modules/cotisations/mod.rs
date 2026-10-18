//! Cotisations : ce que les membres rapportent au groupe, en argent propre
//! et/ou sale. Chacun déclare ses cotisations ; les gradés et leads (ou
//! « Gérer le serveur ») consultent les statistiques, sur Discord et sur le site.

pub mod commands;
pub mod service;

use crate::discord::interaction::Ix;
use anyhow::Result;
use twilight_model::guild::Permissions;

/// Officier : gradé ou lead (rôles du module recrues), ou « Gérer le serveur ».
pub async fn is_officer(ix: &Ix) -> Result<bool> {
    if ix.has_permission(Permissions::MANAGE_GUILD) {
        return Ok(true);
    }
    let Some(guild_id) = ix.guild_id() else {
        return Ok(false);
    };
    let config = crate::modules::recrues::service::get_config(&ix.ctx.db, &guild_id.to_string()).await?;
    Ok(crate::modules::recrues::is_grade(ix, config.as_ref()))
}

/// Montant lisible : « 12 500 $ ».
pub fn money(value: i64) -> String {
    format!("{}\u{202f}$", crate::util::format::format_price(value as f64))
}
