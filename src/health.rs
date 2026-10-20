//! Santé du bot, pour la page « Santé » du site : démarrage, connexion à la
//! gateway, rattrapages des salons de coffre et dernières erreurs. Tout est en
//! mémoire et repart de zéro au redémarrage.
//!
//! Les avertissements et erreurs du journal (`tracing`) sont copiés dans un
//! tampon circulaire borné ([`ErrorLayer`]) : horodatage, cible, message court,
//! serveur s'il est indiqué (champ `guild`, `guild_id` ou `guildId`). Les autres
//! champs (identifiants de membres, contenu des requêtes…) ne sont pas gardés.

use chrono::{DateTime, Utc};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fmt,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};
use tracing::{Event, Level, Subscriber, field::Field};
use tracing_subscriber::{Layer, layer::Context as LayerContext};

/// Événements gardés : les plus anciens sortent du tampon.
pub const ERROR_CAPACITY: usize = 100;
/// Longueur maximale d'un message gardé (caractères).
const MESSAGE_LIMIT: usize = 200;
/// Longueur maximale du résumé de l'erreur jointe (première ligne seulement).
const DETAIL_LIMIT: usize = 160;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Coupe un texte à `limit` caractères (points de suspension).
fn shorten(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(limit.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// Avertissement ou erreur copié du journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogEvent {
    pub at: DateTime<Utc>,
    /// `WARN` ou `ERROR`.
    pub level: &'static str,
    /// Module d'origine (`bot_discord::modules::coffres::ingest`…).
    pub target: String,
    pub message: String,
    /// Serveur concerné, s'il figure dans l'événement.
    pub guild_id: Option<String>,
}

/// Tampon circulaire des derniers avertissements et erreurs.
#[derive(Default)]
pub struct ErrorLog {
    entries: Mutex<VecDeque<LogEvent>>,
}

impl ErrorLog {
    pub fn push(&self, event: LogEvent) {
        let mut entries = lock(&self.entries);
        if entries.len() >= ERROR_CAPACITY {
            entries.pop_front();
        }
        entries.push_back(event);
    }

    /// Du plus récent au plus ancien : ceux du serveur, ou tous (`global`).
    pub fn for_guild(&self, guild_id: &str, global: bool) -> Vec<LogEvent> {
        lock(&self.entries)
            .iter()
            .rev()
            .filter(|event| global || event.guild_id.as_deref() == Some(guild_id))
            .cloned()
            .collect()
    }
}

/// Lecture des champs utiles d'un événement.
#[derive(Default)]
struct Fields {
    message: Option<String>,
    detail: Option<String>,
    guild_id: Option<String>,
}

impl Fields {
    fn record(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = Some(value),
            "error" => self.detail = Some(value),
            "guild" | "guild_id" | "guildId" => self.guild_id = Some(value.trim_matches('"').to_owned()),
            _ => {}
        }
    }
}

impl tracing::field::Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, value.to_owned());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record(field, format!("{value:?}"));
    }
}

/// Couche `tracing` qui copie WARN et ERROR dans un [`ErrorLog`].
pub struct ErrorLayer(Arc<ErrorLog>);

impl ErrorLayer {
    pub fn new(log: Arc<ErrorLog>) -> Self {
        Self(log)
    }
}

/// Événement gardé, à partir des champs lus.
fn log_event(level: Level, target: &str, fields: Fields, at: DateTime<Utc>) -> LogEvent {
    let mut message = fields.message.unwrap_or_default();
    // L'erreur jointe : première ligne seulement (pas de chaîne de causes ni de trace).
    if let Some(detail) = fields.detail.as_deref().and_then(|d| d.lines().map(str::trim).find(|l| !l.is_empty())) {
        let detail = shorten(detail, DETAIL_LIMIT);
        message = if message.is_empty() { detail } else { format!("{message} : {detail}") };
    }
    LogEvent {
        at,
        level: if level == Level::ERROR { "ERROR" } else { "WARN" },
        target: target.to_owned(),
        message: shorten(&message, MESSAGE_LIMIT),
        guild_id: fields.guild_id.filter(|g| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit())),
    }
}

impl<S: Subscriber> Layer<S> for ErrorLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: LayerContext<'_, S>) {
        let metadata = event.metadata();
        // WARN et ERROR seulement (les niveaux plus bavards sont « plus grands »).
        if *metadata.level() > Level::WARN {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        self.0.push(log_event(*metadata.level(), metadata.target(), fields, Utc::now()));
    }
}

/// État d'un shard, relevé à chaque événement reçu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardHealth {
    pub id: u32,
    pub connected: bool,
    /// Latence moyenne des derniers battements de cœur, en millisecondes.
    pub latency_ms: Option<u64>,
    pub seen_at: DateTime<Utc>,
}

/// Dernier rattrapage d'un salon de coffre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatchUp {
    pub at: DateTime<Utc>,
    pub ok: bool,
}

/// Santé du processus (dans `Ctx`).
pub struct Health {
    pub started: Instant,
    pub started_at: DateTime<Utc>,
    pub errors: Arc<ErrorLog>,
    shards: Mutex<BTreeMap<u32, ShardHealth>>,
    catch_ups: Mutex<HashMap<String, CatchUp>>,
}

impl Health {
    pub fn new(errors: Arc<ErrorLog>) -> Self {
        Self {
            started: Instant::now(),
            started_at: Utc::now(),
            errors,
            shards: Mutex::default(),
            catch_ups: Mutex::default(),
        }
    }

    /// Relève l'état d'un shard (appelé par la boucle de réception).
    pub fn observe_shard(&self, shard: &twilight_gateway::Shard) {
        let state = ShardHealth {
            id: shard.id().number(),
            connected: matches!(shard.state(), twilight_gateway::ShardState::Active),
            latency_ms: shard.latency().average().map(|d| d.as_millis() as u64),
            seen_at: Utc::now(),
        };
        lock(&self.shards).insert(state.id, state);
    }

    pub fn shards(&self) -> Vec<ShardHealth> {
        lock(&self.shards).values().cloned().collect()
    }

    pub fn record_catch_up(&self, channel_id: &str, ok: bool) {
        lock(&self.catch_ups).insert(channel_id.to_owned(), CatchUp { at: Utc::now(), ok });
    }

    pub fn catch_up(&self, channel_id: &str) -> Option<CatchUp> {
        lock(&self.catch_ups).get(channel_id).cloned()
    }

    /// Secondes depuis le démarrage.
    pub fn uptime_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(Arc::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt as _;

    fn event(guild: Option<&str>, message: &str) -> LogEvent {
        LogEvent {
            at: Utc::now(),
            level: "WARN",
            target: "test".into(),
            message: message.into(),
            guild_id: guild.map(str::to_owned),
        }
    }

    #[test]
    fn ring_buffer_is_bounded_and_filtered_by_guild() {
        let log = ErrorLog::default();
        for i in 0..ERROR_CAPACITY + 5 {
            log.push(event(Some(if i % 2 == 0 { "1" } else { "2" }), &i.to_string()));
        }
        log.push(event(None, "sans serveur"));
        let all = log.for_guild("1", true);
        assert_eq!(all.len(), ERROR_CAPACITY);
        assert_eq!(all[0].message, "sans serveur", "le plus récent en premier");
        let mine = log.for_guild("1", false);
        assert!(mine.iter().all(|e| e.guild_id.as_deref() == Some("1")));
        assert!(!mine.iter().any(|e| e.guild_id.is_none()), "hors vue globale : rien sans serveur");
    }

    #[test]
    fn layer_keeps_warnings_and_errors_only() {
        let log = Arc::new(ErrorLog::default());
        let subscriber = tracing_subscriber::registry().with(ErrorLayer::new(Arc::clone(&log)));
        tracing::subscriber::with_default(subscriber, || {
            let guild_id = 42_u64;
            let error = anyhow::anyhow!("salon introuvable").context("lecture du salon");
            tracing::info!("démarrage");
            tracing::warn!(%guild_id, user = "secret", ?error, "rattrapage impossible");
            tracing::error!(guild = "7", "erreur de l'API");
            tracing::warn!(guild_id = "pas un serveur", "{}", "x".repeat(500));
        });
        let all = log.for_guild("", true);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].message, "rattrapage impossible : lecture du salon");
        assert_eq!(all[2].guild_id.as_deref(), Some("42"));
        assert!(!all[2].message.contains("secret"), "les autres champs ne sont pas gardés");
        assert_eq!((all[1].level, all[1].guild_id.as_deref()), ("ERROR", Some("7")));
        assert_eq!(all[0].guild_id, None);
        assert_eq!(all[0].message.chars().count(), MESSAGE_LIMIT);
    }

    #[test]
    fn catch_ups_are_remembered() {
        let health = Health::default();
        assert!(health.catch_up("1").is_none());
        health.record_catch_up("1", false);
        health.record_catch_up("1", true);
        assert!(health.catch_up("1").unwrap().ok);
    }
}
