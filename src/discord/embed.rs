//! Petits utilitaires autour d'`EmbedBuilder`.

use crate::util::time::DbTime;
use twilight_model::{
    channel::message::embed::{EmbedAuthor, EmbedFooter},
    util::Timestamp,
};
use twilight_util::builder::embed::{EmbedAuthorBuilder, EmbedFooterBuilder, ImageSource};

pub use twilight_util::builder::embed::EmbedBuilder;

pub fn footer(text: impl Into<String>) -> EmbedFooter {
    EmbedFooterBuilder::new(text).build()
}

pub fn author(name: impl Into<String>) -> EmbedAuthor {
    EmbedAuthorBuilder::new(name).build()
}

/// Une URL d'image invalide ne doit jamais empêcher l'envoi de l'embed.
pub fn image(url: &str) -> Option<ImageSource> {
    ImageSource::url(url).ok()
}

pub fn timestamp(time: DbTime) -> Timestamp {
    Timestamp::from_secs(time.unix()).expect("date dans les bornes Discord")
}

pub fn now() -> Timestamp {
    timestamp(DbTime::now())
}

pub trait EmbedBuilderExt {
    fn maybe_thumbnail(self, url: Option<String>) -> Self;
}

impl EmbedBuilderExt for EmbedBuilder {
    fn maybe_thumbnail(self, url: Option<String>) -> Self {
        match url.as_deref().and_then(image) {
            Some(source) => self.thumbnail(source),
            None => self,
        }
    }
}
