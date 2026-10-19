//! Tickets : réimplémentation de Dragory/modmailbot.
//!
//! Compatible avec l'original : mêmes options (`config.ini` ou variables
//! `MM_*`), mêmes commandes et alias, mêmes messages, même schéma de base
//! (`knex_migrations` compris) et même format de logs texte. Seuls les plugins
//! JavaScript ne sont pas pris en charge.

mod attachments;
mod auto_close;
pub mod buttons;
mod commands;
mod config;
mod context_menu;
mod db;
mod formatters;
mod handlers;
mod ini;
mod logs;
mod models;
mod parser;
mod staff;
mod store;
mod thread;
mod util;
mod web;
pub(crate) mod web_api;

use crate::context::{Context, Ctx};
use anyhow::Result;
use config::MmConfig;
use sqlx::SqlitePool;
pub use staff::TicketStats;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use twilight_gateway::Intents;
use twilight_model::{
    gateway::{
        payload::outgoing::UpdatePresence,
        presence::{Activity, ActivityType, MinimalActivity, Status},
    },
    id::{
        Id,
        marker::{ChannelMarker, UserMarker},
    },
};

/// Pages web du module (logs, liste des tickets, pièces jointes).
pub fn web_routes() -> axum::Router<Context> {
    web::routes()
}

pub struct Modmail {
    pub config: MmConfig,
    pub db: SqlitePool,
    /// Client HTTP pour les pièces jointes et l'IP publique.
    http: reqwest::Client,
    /// File des messages privés : plusieurs messages rapprochés ne créent pas plusieurs tickets.
    dm_queue: tokio::sync::Mutex<()>,
    creation_lock: tokio::sync::Mutex<()>,
    dm_channels: Mutex<HashMap<Id<UserMarker>, Id<ChannelMarker>>>,
    auto_alerts: Mutex<HashMap<String, tokio::task::AbortHandle>>,
    /// Membres bannis : le départ qui suit le bannissement n'est pas signalé une seconde fois.
    leave_ignore: Mutex<HashSet<Id<UserMarker>>>,
    public_ip: tokio::sync::OnceCell<String>,
}

impl Modmail {
    /// Charge la configuration ; `None` si le module n'est pas configuré.
    pub async fn load() -> Result<Option<Self>> {
        let Some(config) = MmConfig::load()? else {
            tracing::info!("module ticket désactivé (ni config.ini ni variables MM_*)");
            return Ok(None);
        };
        let db = db::open(&config.sqlite_filename).await?;
        tracing::info!(path = %config.sqlite_filename.display(), inbox = %config.inbox_server_id, "module ticket activé");
        Ok(Some(Self {
            config,
            db,
            http: crate::util::http::client(),
            dm_queue: tokio::sync::Mutex::new(()),
            creation_lock: tokio::sync::Mutex::new(()),
            dm_channels: Mutex::new(HashMap::new()),
            auto_alerts: Mutex::new(HashMap::new()),
            leave_ignore: Mutex::new(HashSet::new()),
            public_ip: tokio::sync::OnceCell::new(),
        }))
    }

    pub fn intents(&self) -> Intents {
        let mut intents = Intents::GUILD_MESSAGES
            | Intents::DIRECT_MESSAGES
            | Intents::MESSAGE_CONTENT
            | Intents::GUILD_MODERATION
            | Intents::GUILD_VOICE_STATES;
        if self.config.notify_on_reaction || self.config.notify_on_reaction_removal {
            intents |= Intents::DIRECT_MESSAGE_REACTIONS;
        }
        if self.config.typing_proxy {
            intents |= Intents::DIRECT_MESSAGE_TYPING;
        }
        if self.config.typing_proxy_reverse {
            intents |= Intents::GUILD_MESSAGE_TYPING;
        }
        intents
    }

    /// Démarre les tâches périodiques et le statut du bot.
    pub fn start(&self, ctx: Context) {
        let periodic = Arc::clone(&ctx);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(2));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let modmail = periodic.modmail.as_ref().expect("module ticket");
                if let Err(error) = modmail.apply_scheduled_closes(&periodic).await {
                    tracing::error!(?error, "fermetures programmées");
                }
                if modmail.config.allow_suspend
                    && let Err(error) = modmail.apply_scheduled_suspensions(&periodic).await
                {
                    tracing::error!(?error, "suspensions programmées");
                }
                if modmail.config.allow_block
                    && let Err(error) = modmail.remove_expired_blocks(&periodic).await
                {
                    tracing::error!(?error, "blocages expirés");
                }
            }
        });

        // Tickets inactifs (`autoCloseInactive`) : une vérification par minute suffit.
        if self.config.auto_close_inactive_ms.is_some() {
            let inactivity = Arc::clone(&ctx);
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(60));
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    ticker.tick().await;
                    let modmail = inactivity.modmail.as_ref().expect("module ticket");
                    if let Err(error) = modmail.warn_inactive_threads(&inactivity).await {
                        tracing::error!(?error, "fermeture des tickets inactifs");
                    }
                }
            });
        }

        let startup = Arc::clone(&ctx);
        tokio::spawn(async move {
            // Le temps que les guildes arrivent dans le cache.
            tokio::time::sleep(Duration::from_secs(15)).await;
            let modmail = startup.modmail.as_ref().expect("module ticket");
            match store::open_threads(&modmail.db).await {
                Ok(threads) => {
                    for thread in threads {
                        if let Err(error) = modmail.recover_downtime_messages(&startup, &thread).await {
                            tracing::error!(user = %thread.user_id, ?error, "récupération des messages manqués");
                        }
                    }
                }
                Err(error) => tracing::error!(?error, "lecture des tickets ouverts"),
            }
            // Le statut est réappliqué toutes les heures : il peut sauter.
            loop {
                modmail.apply_status(&startup);
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }

    fn apply_status(&self, ctx: &Ctx) {
        let Some(status) = &self.config.status else {
            return;
        };
        let kind = match self.config.status_type.as_str() {
            "watching" => ActivityType::Watching,
            "listening" => ActivityType::Listening,
            "streaming" => ActivityType::Streaming,
            "custom" => ActivityType::Custom,
            "competing" => ActivityType::Competing,
            _ => ActivityType::Playing,
        };
        let mut activity: Activity = MinimalActivity {
            kind,
            name: if kind == ActivityType::Custom { "custom".to_owned() } else { status.clone() },
            url: (kind == ActivityType::Streaming).then(|| self.config.status_url.clone()),
        }
        .into();
        if kind == ActivityType::Custom {
            activity.state = Some(status.clone());
        }
        let Ok(presence) = UpdatePresence::new(vec![activity], false, None, Status::Online) else {
            return;
        };
        for sender in ctx.shard_senders.get().into_iter().flatten() {
            let _ = sender.command(&presence);
        }
    }

    async fn apply_scheduled_closes(&self, ctx: &Ctx) -> Result<()> {
        for thread in store::threads_to_close(&self.db).await? {
            let silent = thread.scheduled_close_silent.unwrap_or(0) != 0;
            if let (Some(message), false) = (&self.config.close_message, silent) {
                let _ = self.send_system_message_to_user(ctx, &thread, message, true).await;
            }
            self.close_thread(ctx, &thread, false, silent).await?;
            let body = if Self::is_auto_close(ctx, &thread) {
                format!(
                    "Ticket modmail n°{} avec {} ({}) fermé automatiquement pour inactivité",
                    thread.number(),
                    thread.user_name,
                    thread.user_id
                )
            } else {
                format!(
                    "Ticket modmail n°{} avec {} ({}) fermé comme programmé par {}",
                    thread.number(),
                    thread.user_name,
                    thread.user_id,
                    thread.scheduled_close_name.clone().unwrap_or_default()
                )
            };
            self.send_close_notification(ctx, &thread, &body).await?;
        }
        Ok(())
    }

    async fn apply_scheduled_suspensions(&self, ctx: &Ctx) -> Result<()> {
        for thread in store::threads_to_suspend(&self.db).await? {
            if !thread.is_open() {
                continue;
            }
            store::suspend(&self.db, &thread.id).await?;
            self.post_system_message(
                ctx,
                &thread,
                &format!(
                    "**Ticket suspendu** comme programmé par {}. Il sera considéré comme fermé jusqu'à sa réactivation avec `{}unsuspend`",
                    thread.scheduled_suspend_name.clone().unwrap_or_default(),
                    self.config.prefix
                ),
            )
            .await?;
        }
        Ok(())
    }

    async fn remove_expired_blocks(&self, ctx: &Ctx) -> Result<()> {
        for user_id in store::expired_blocks(&self.db).await? {
            store::unblock(&self.db, &user_id).await?;
            let mentions = crate::discord::messages::Mentions::Explicit {
                users: crate::discord::messages::parse_id(&user_id).into_iter().collect(),
                roles: Vec::new(),
                everyone: false,
            };
            let allowed = mentions.to_allowed();
            let _ = ctx
                .http
                .create_message(self.config.log_channel_id)
                .content(&format!("Le blocage de <@!{user_id}> (id `{user_id}`) a expiré"))
                .allowed_mentions(Some(&allowed))
                .await;
        }
        Ok(())
    }
}
