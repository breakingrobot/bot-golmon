use super::{channels::ensure_channel, service};
use crate::{context::Ctx, discord::messages::parse_id};
use twilight_model::{
    gateway::payload::incoming::MemberUpdate,
    id::{Id, marker::RoleMarker},
};

/// Ouvre (ou ferme) le suivi d'une recrue quand le rôle configuré lui est
/// attribué ou retiré. Le salon privé est conservé quand le rôle est retiré :
/// l'historique de la période d'essai reste consultable.
///
/// `previous_roles` vient du cache avant la mise à jour ; un membre absent du
/// cache est ignoré (un simple changement de pseudo passerait sinon pour une
/// attribution de rôle).
pub async fn on_member_update(ctx: &Ctx, update: &MemberUpdate, previous_roles: &[Id<RoleMarker>]) {
    let result = async {
        let guild = update.guild_id.to_string();
        let Some(role_id) =
            service::get_config(&ctx.db, &guild).await?.and_then(|c| c.recrue_role_id).as_deref().and_then(parse_id)
        else {
            return anyhow::Ok(());
        };
        let had = previous_roles.contains(&role_id);
        let has = update.roles.contains(&role_id);
        if had == has {
            return Ok(());
        }

        let session = service::ensure_current_session(ctx, update.guild_id).await?;
        if has {
            ensure_channel(ctx, update.guild_id, update.user.id).await?;
            service::add_entry(&ctx.db, &session.id, &guild, &update.user.id.to_string()).await?;
        } else {
            service::remove_entry(&ctx.db, &session.id, &update.user.id.to_string()).await?;
        }
        service::sync_dashboard(ctx, update.guild_id).await?;
        tracing::info!(user = %update.user.name, added = has, "suivi de la recrue mis à jour");
        Ok(())
    }
    .await;
    if let Err(error) = result {
        tracing::error!(user = %update.user.id, ?error, "suivi de la recrue échoué");
    }
}
