use crate::{config::Config, modmail::Modmail, modules::vehicles::api::VehicleApi};
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock};
use twilight_cache_inmemory::DefaultInMemoryCache;
use twilight_gateway::MessageSender;
use twilight_http::{Client as HttpClient, client::InteractionClient};
use twilight_model::id::{
    Id,
    marker::{ApplicationMarker, GuildMarker, UserMarker},
};

/// État partagé par toutes les tâches. Tout est derrière un `Arc` unique :
/// chaque handler ne clone qu'un pointeur.
pub struct Ctx {
    pub config: Config,
    pub http: HttpClient,
    pub cache: DefaultInMemoryCache,
    pub db: SqlitePool,
    pub application_id: Id<ApplicationMarker>,
    pub bot_user_id: Id<UserMarker>,
    pub bot_username: String,
    pub vehicles: VehicleApi,
    pub modmail: Option<Modmail>,
    /// *Message Content Intent* activé sur l'application : le bot peut lire le
    /// contenu (et les encadrés) des messages, pour le module coffre.
    pub message_content: bool,
    /// Un émetteur par shard, pour les commandes gateway (demande de membres).
    pub shard_senders: OnceLock<Vec<MessageSender>>,
}

pub type Context = Arc<Ctx>;

impl Ctx {
    pub fn interaction(&self) -> InteractionClient<'_> {
        self.http.interaction(self.application_id)
    }

    /// Émetteur du shard qui gère `guild_id` (formule de sharding Discord).
    pub fn sender_for(&self, guild_id: Id<GuildMarker>) -> Option<&MessageSender> {
        let senders = self.shard_senders.get()?;
        let shard = (guild_id.get() >> 22) % senders.len() as u64;
        senders.get(shard as usize)
    }
}
