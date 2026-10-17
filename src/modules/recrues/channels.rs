//! Salon privé d'essai de chaque recrue.

use super::service;
use crate::{
    context::Ctx,
    discord::{
        members::fetch_display_name,
        messages::{self, Mentions, Outgoing, parse_id},
    },
};
use anyhow::Result;
use twilight_http::request::AuditLogReason as _;
use twilight_model::{
    channel::{
        ChannelType,
        permission_overwrite::{PermissionOverwrite, PermissionOverwriteType},
    },
    guild::Permissions,
    id::{
        Id,
        marker::{GuildMarker, UserMarker},
    },
};

/// Droits accordés à la recrue, aux gradés et aux leads dans le salon privé.
const MEMBER_ALLOW: Permissions = Permissions::VIEW_CHANNEL
    .union(Permissions::SEND_MESSAGES)
    .union(Permissions::READ_MESSAGE_HISTORY)
    .union(Permissions::ATTACH_FILES)
    .union(Permissions::EMBED_LINKS);

pub enum EnsureChannel {
    Ok { created: bool, channel_id: String },
    Failed(String),
}

/// Discord n'accepte que minuscules, chiffres et tirets dans un nom de salon.
pub fn slugify(name: &str) -> String {
    let ascii = deunicode::deunicode(name).to_lowercase();
    let mut slug = String::new();
    for ch in ascii.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(90).collect();
    if slug.is_empty() { "recrue".to_owned() } else { slug }
}

/// Crée (ou retrouve) le salon privé d'une recrue dans la catégorie configurée.
/// Idempotent : si le salon mémorisé existe encore, il est renvoyé tel quel.
pub async fn ensure_channel(ctx: &Ctx, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<EnsureChannel> {
    let guild = guild_id.to_string();
    let config = service::get_config(&ctx.db, &guild).await?;
    let Some(config) = config else {
        return Ok(EnsureChannel::Failed(
            "Aucune catégorie configurée pour les salons des recrues (`/recrue config`).".to_owned(),
        ));
    };
    let Some(category_id) = config.category_id.as_deref().and_then(parse_id) else {
        return Ok(EnsureChannel::Failed(
            "Aucune catégorie configurée pour les salons des recrues (`/recrue config`).".to_owned(),
        ));
    };

    if let Some(known) = service::channel_id_of(&ctx.db, &guild, &user_id.to_string()).await?
        && let Some(channel_id) = parse_id(&known)
    {
        match ctx.http.channel(channel_id).await {
            Ok(_) => return Ok(EnsureChannel::Ok { created: false, channel_id: known }),
            // Salon supprimé à la main : on en recrée un.
            Err(error) if messages::is_not_found(&error) => {}
            Err(error) => return Err(error.into()),
        }
    }

    let bot_permissions = ctx.cache.permissions().root(ctx.bot_user_id, guild_id).unwrap_or(Permissions::empty());
    if !bot_permissions.contains(Permissions::MANAGE_CHANNELS) && !bot_permissions.contains(Permissions::ADMINISTRATOR)
    {
        return Ok(EnsureChannel::Failed("Le bot n'a pas la permission « Gérer les salons ».".to_owned()));
    }

    let name = fetch_display_name(ctx, guild_id, user_id).await;
    let mut overwrites = vec![
        PermissionOverwrite {
            id: guild_id.cast(),
            kind: PermissionOverwriteType::Role,
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
        },
        PermissionOverwrite {
            id: user_id.cast(),
            kind: PermissionOverwriteType::Member,
            allow: MEMBER_ALLOW,
            deny: Permissions::empty(),
        },
        // Sans cette entrée, le refus de @everyone retire au bot l'accès à son propre
        // salon (sauf s'il est administrateur) : pas de message d'accueil, ni de suppression.
        PermissionOverwrite {
            id: ctx.bot_user_id.cast(),
            kind: PermissionOverwriteType::Member,
            allow: MEMBER_ALLOW,
            deny: Permissions::empty(),
        },
    ];
    for role in [&config.grade_role_id, &config.lead_role_id].into_iter().flatten().filter_map(|r| parse_id(r)) {
        overwrites.push(PermissionOverwrite {
            id: role,
            kind: PermissionOverwriteType::Role,
            allow: MEMBER_ALLOW,
            deny: Permissions::empty(),
        });
    }

    let channel_name = format!("essai-{}", slugify(name.as_deref().unwrap_or(&user_id.to_string())));
    let reason = format!("Salon d'essai de la recrue {}", name.as_deref().unwrap_or(&user_id.to_string()));
    let created = ctx
        .http
        .create_guild_channel(guild_id, &channel_name)
        .kind(ChannelType::GuildText)
        .parent_id(category_id)
        .permission_overwrites(&overwrites)
        .reason(&reason)
        .await;
    let channel = match created {
        Ok(response) => response.model().await?,
        Err(error) => {
            tracing::error!(%user_id, %error, "création du salon de la recrue échouée");
            return Ok(EnsureChannel::Failed(
                "Création du salon impossible. Vérifie que la catégorie existe encore et qu'elle n'est pas pleine (50 salons maximum)."
                    .to_owned(),
            ));
        }
    };

    service::remember_channel(&ctx.db, &guild, &user_id.to_string(), &channel.id.to_string()).await?;

    let staff = if config.grade_role_id.is_some() || config.lead_role_id.is_some() {
        "les gradés et les leads"
    } else {
        "l'encadrement"
    };
    let welcome = Outgoing::text(format!(
        "Bienvenue <@{user_id}> 👋\nCe salon est privé : seuls toi et {staff} y ont accès. Pose ici tes questions pendant ta période d'essai."
    ))
    .with_mentions(Mentions::Explicit { users: vec![user_id], roles: Vec::new(), everyone: false });
    if let Err(error) = messages::send(ctx, channel.id, &welcome).await {
        tracing::warn!(%user_id, ?error, "message d'accueil non envoyé dans le salon de la recrue");
    }
    tracing::info!(channel = %channel_name, %user_id, "salon d'essai créé");

    Ok(EnsureChannel::Ok { created: true, channel_id: channel.id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugifies_display_names() {
        assert_eq!(slugify("Élodie La Bête!!"), "elodie-la-bete");
        assert_eq!(slugify("★★★"), "recrue");
        assert_eq!(slugify("  John_Doe  "), "john-doe");
    }
}
