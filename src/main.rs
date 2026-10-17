//! Bot Discord : présences, absences, transactions, véhicules, recrues,
//! annuaire et tickets (modmail compatible Dragory/modmailbot).

mod config;
mod context;
mod db;
mod discord;
mod modmail;
mod modules;
mod util;
mod web;

use anyhow::{Context as _, Result};
use context::Ctx;
use std::sync::{Arc, OnceLock};
use tokio::sync::watch;
use twilight_cache_inmemory::{DefaultInMemoryCache, ResourceType};
use twilight_gateway::Config as GatewayConfig;
use twilight_http::Client as HttpClient;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,twilight_gateway=warn,twilight_http_ratelimiting=warn,sqlx=warn".into()),
        )
        .init();
    rustls::crypto::ring::default_provider().install_default().ok();

    let deploy_only = std::env::args().nth(1).as_deref() == Some("deploy-commands");

    let config = config::Config::from_env()?;
    let db = db::open_main(&config.database_path).await?;
    tracing::info!(path = %config.database_path.display(), "base de données prête");

    let modmail = modmail::Modmail::load().await?;

    let http = HttpClient::new(config.token.clone());
    let application =
        http.current_user_application().await?.model().await.context("lecture de l'application Discord")?;
    let bot_user = http.current_user().await?.model().await?;

    // Cache minimal : guildes, salons, rôles et membres (rôles des présences,
    // recrues, permissions modmail). Ni messages, ni présences, ni emojis.
    let mut resources = ResourceType::GUILD
        | ResourceType::CHANNEL
        | ResourceType::ROLE
        | ResourceType::MEMBER
        | ResourceType::USER
        | ResourceType::USER_CURRENT;
    if modmail.is_some() {
        resources |= ResourceType::VOICE_STATE;
    }
    let cache = DefaultInMemoryCache::builder().resource_types(resources).message_cache_size(0).build();

    let ctx = Arc::new(Ctx {
        config,
        http,
        cache,
        db,
        application_id: application.id,
        bot_user_id: bot_user.id,
        bot_username: bot_user.name.clone(),
        vehicles: modules::vehicles::api::VehicleApi::new(),
        modmail,
        shard_senders: OnceLock::new(),
    });

    if ctx.config.deploy_commands || deploy_only {
        discord::commands::deploy(&ctx).await.context("déploiement des slash commands")?;
    }
    if deploy_only {
        return Ok(());
    }

    let intents = discord::gateway::intents(&ctx);
    let gateway_config = GatewayConfig::new(ctx.config.token.clone(), intents);
    let shards: Vec<_> = twilight_gateway::create_recommended(&ctx.http, gateway_config, |_, builder| builder.build())
        .await
        .context("connexion à la gateway Discord")?
        .collect();
    let _ = ctx.shard_senders.set(shards.iter().map(twilight_gateway::Shard::sender).collect());
    tracing::info!(shards = shards.len(), ?intents, "démarrage des shards");

    modules::presences::cron::spawn(Arc::clone(&ctx));
    modules::recrues::cron::spawn(Arc::clone(&ctx));
    if let Some(modmail) = &ctx.modmail {
        modmail.start(Arc::clone(&ctx));
    }
    web::spawn(Arc::clone(&ctx));

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let tasks: Vec<_> = shards
        .into_iter()
        .map(|shard| tokio::spawn(discord::gateway::run_shard(Arc::clone(&ctx), shard, shutdown_rx.clone())))
        .collect();

    shutdown_signal().await;
    tracing::info!("arrêt demandé, fermeture des connexions");
    let _ = shutdown_tx.send(true);
    for task in tasks {
        let _ = task.await;
    }
    ctx.db.close().await;
    if let Some(modmail) = &ctx.modmail {
        modmail.db.close().await;
    }
    Ok(())
}

/// Ctrl+C en local, SIGTERM en conteneur (arrêt d'un déploiement Northflank).
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal SIGTERM");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
