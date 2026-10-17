//! Configuration lue dans l'environnement (et un éventuel fichier `.env`).

use anyhow::{Context as _, Result, bail};
use std::{env, path::PathBuf};
use twilight_model::id::{Id, marker::GuildMarker};

#[derive(Clone, Debug)]
pub struct Config {
    pub token: String,
    /// Liste blanche des guildes. Vide = pas de restriction (pratique en dev).
    pub guild_whitelist: Vec<Id<GuildMarker>>,
    pub database_path: PathBuf,
    /// Déploie les slash commands au démarrage (idempotent, activé par défaut).
    pub deploy_commands: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let token = env::var("DISCORD_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty())
            // Compatibilité modmailbot : le token peut aussi venir de MM_TOKEN.
            .or_else(|| env::var("MM_TOKEN").ok().filter(|v| !v.trim().is_empty()))
            .context("DISCORD_TOKEN manquant dans les variables d'environnement")?;

        let guild_whitelist = env::var("DISCORD_GUILD_IDS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| id.parse().with_context(|| format!("DISCORD_GUILD_IDS : identifiant invalide « {id} »")))
            .collect::<Result<Vec<_>>>()?;

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "file:./prisma/dev.db".to_owned());
        let database_path = PathBuf::from(
            database_url
                .trim_matches('"')
                .trim_start_matches("sqlite://")
                .trim_start_matches("sqlite:")
                .trim_start_matches("file:"),
        );
        if database_path.as_os_str().is_empty() {
            bail!("DATABASE_URL invalide : {database_url}");
        }

        let deploy_commands = env::var("DEPLOY_COMMANDS")
            .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "off" | "no"))
            .unwrap_or(true);

        Ok(Self { token, guild_whitelist, database_path, deploy_commands })
    }

    pub fn is_guild_allowed(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_whitelist.is_empty() || self.guild_whitelist.contains(&guild_id)
    }
}
