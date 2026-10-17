//! Configuration lue dans l'environnement (et un éventuel fichier `.env`).

use anyhow::{Context as _, Result, bail};
use std::{
    env,
    path::{Path, PathBuf},
};

use twilight_model::id::{Id, marker::GuildMarker};

const DEFAULT_DATABASE_PATH: &str = "data/bot.db";

#[derive(Clone, Debug)]
pub struct Config {
    pub token: String,
    /// Liste blanche des guildes. Vide = pas de restriction (pratique en dev).
    pub guild_whitelist: Vec<Id<GuildMarker>>,
    pub database_path: PathBuf,
    /// Déploie les slash commands au démarrage (idempotent, activé par défaut).
    pub deploy_commands: bool,
    pub web: WebConfig,
}

/// Serveur web (logs des tickets, statistiques, `/healthz`).
#[derive(Clone, Debug, Default)]
pub struct WebConfig {
    /// `PORT` : port d'écoute quand le module ticket ne le fixe pas.
    pub port: Option<u16>,
    /// `WEB_URL` : adresse publique (à défaut, `url` du module ticket).
    pub public_url: Option<String>,
    /// `DISCORD_CLIENT_SECRET` : active la connexion Discord des pages web.
    pub client_secret: Option<String>,
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

        let database_path = match env::var("DATABASE_URL").ok().filter(|v| !v.trim().is_empty()) {
            Some(database_url) => {
                let path = PathBuf::from(
                    database_url
                        .trim_matches('"')
                        .trim_start_matches("sqlite://")
                        .trim_start_matches("sqlite:")
                        .trim_start_matches("file:"),
                );
                if path.as_os_str().is_empty() {
                    bail!("DATABASE_URL invalide : {database_url}");
                }
                path
            }
            None => default_database_path()?,
        };

        let deploy_commands = env::var("DEPLOY_COMMANDS")
            .map(|v| !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "off" | "no"))
            .unwrap_or(true);

        let non_empty = |key: &str| env::var(key).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let web = WebConfig {
            port: non_empty("PORT").map(|p| p.parse().with_context(|| format!("PORT invalide : {p}"))).transpose()?,
            public_url: non_empty("WEB_URL").map(|u| u.trim_end_matches('/').to_owned()),
            client_secret: non_empty("DISCORD_CLIENT_SECRET"),
        };

        Ok(Self { token, guild_whitelist, database_path, deploy_commands, web })
    }

    pub fn is_guild_allowed(&self, guild_id: Id<GuildMarker>) -> bool {
        self.guild_whitelist.is_empty() || self.guild_whitelist.contains(&guild_id)
    }
}

/// Base principale par défaut, avec reprise des emplacements de la version TypeScript :
/// - `data/dev.db` (ancien volume remonté sur `/app/data`) est renommée en `data/bot.db` ;
/// - `prisma/dev.db` (volume encore monté sur `/app/prisma`) est utilisée telle quelle.
fn default_database_path() -> Result<PathBuf> {
    let path = PathBuf::from(DEFAULT_DATABASE_PATH);
    if path.exists() {
        return Ok(path);
    }
    let legacy = Path::new("data/dev.db");
    if legacy.exists() {
        // Les fichiers -wal/-shm suivent la base, sinon des écritures récentes seraient perdues.
        for suffix in ["", "-wal", "-shm"] {
            let from = PathBuf::from(format!("data/dev.db{suffix}"));
            if from.exists() {
                std::fs::rename(&from, format!("{DEFAULT_DATABASE_PATH}{suffix}"))
                    .with_context(|| format!("renommage de {}", from.display()))?;
            }
        }
        tracing::info!("base data/dev.db renommée en {DEFAULT_DATABASE_PATH}");
        return Ok(path);
    }
    let legacy = PathBuf::from("prisma/dev.db");
    if legacy.exists() {
        tracing::warn!("base trouvée dans prisma/dev.db : monter le volume sur /app/data (voir README)");
        return Ok(legacy);
    }
    Ok(path)
}
