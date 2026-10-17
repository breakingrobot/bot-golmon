//! Stockage des pièces jointes (`attachmentStorage` : original, local, discord).

use super::{Modmail, config::AttachmentStorage};
use crate::context::Ctx;
use anyhow::{Result, bail};
use std::path::PathBuf;
use twilight_model::{channel::Attachment, http::attachment::Attachment as Upload};

/// Fichier téléchargé, prêt à être renvoyé sur Discord.
#[derive(Clone)]
pub struct FileData {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl FileData {
    pub fn to_uploads(files: &[FileData]) -> Vec<Upload> {
        files
            .iter()
            .enumerate()
            .map(|(index, file)| Upload::from_bytes(file.name.clone(), file.bytes.clone(), index as u64))
            .collect()
    }
}

impl Modmail {
    pub fn local_attachment_path(&self, attachment_id: &str) -> PathBuf {
        self.config.attachment_dir.join(attachment_id)
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>> {
        let mut last_error = None;
        for _ in 0..3 {
            match self
                .http
                .get(url)
                .timeout(std::time::Duration::from_secs(60))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            {
                Ok(response) => return Ok(response.bytes().await?.to_vec()),
                Err(error) => {
                    tracing::warn!(%error, "téléchargement de pièce jointe échoué, nouvel essai");
                    last_error = Some(error);
                }
            }
        }
        bail!(
            "Échec du téléchargement de la pièce jointe après 3 tentatives : {}",
            last_error.map(|e| e.to_string()).unwrap_or_default()
        )
    }

    /// Les pièces jointes au nom identique sont ignorées par l'API (« unknown.png »
    /// collé plusieurs fois) : chaque fichier est renommé d'après son identifiant.
    pub async fn attachment_to_file(&self, attachment: &Attachment) -> Result<FileData> {
        let bytes = self.download(&attachment.url).await?;
        let ext = std::path::Path::new(&attachment.filename)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_else(|| ".dat".to_owned());
        Ok(FileData { name: format!("{}{ext}", attachment.id), bytes })
    }

    /// Enregistre une pièce jointe selon `attachmentStorage` et renvoie l'URL à afficher.
    pub async fn save_attachment(&self, ctx: &Ctx, attachment: &Attachment) -> String {
        match self.try_save_attachment(ctx, attachment).await {
            Ok(url) => url,
            Err(error) => {
                tracing::error!(%error, "pièce jointe non enregistrée");
                format!("Impossible d'enregistrer la pièce jointe : {error}")
            }
        }
    }

    async fn try_save_attachment(&self, ctx: &Ctx, attachment: &Attachment) -> Result<String> {
        match self.config.attachment_storage {
            AttachmentStorage::Original => Ok(attachment.url.clone()),
            AttachmentStorage::Local => {
                let path = self.local_attachment_path(&attachment.id.to_string());
                if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                    let bytes = self.download(&attachment.url).await?;
                    if let Some(parent) = path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&path, bytes).await?;
                }
                Ok(self
                    .self_url(&format!("attachments/{}/{}", attachment.id, sanitize_filename(&attachment.filename)))
                    .await)
            }
            AttachmentStorage::Discord => {
                if attachment.size > 8 * 1024 * 1024 {
                    return Ok(
                        "Impossible d'enregistrer la pièce jointe : fichier trop volumineux (8 Mo max)".to_owned()
                    );
                }
                let channel_id = self
                    .config
                    .attachment_storage_channel_id
                    .ok_or_else(|| anyhow::anyhow!("Salon de stockage des pièces jointes introuvable !"))?;
                let file = self.attachment_to_file(attachment).await?;
                let uploads = FileData::to_uploads(&[file]);
                for _ in 0..3 {
                    match ctx.http.create_message(channel_id).attachments(&uploads).await {
                        Ok(response) => {
                            let message = response.model().await?;
                            if let Some(saved) = message.attachments.first() {
                                return Ok(saved.url.clone());
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%error, "copie de la pièce jointe dans le salon de stockage échouée")
                        }
                    }
                }
                Ok("Impossible d'enregistrer la pièce jointe".to_owned())
            }
        }
    }
}

/// Le serveur web n'accepte que `[0-9a-z._-]` dans le nom servi.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect();
    if cleaned.is_empty() { "file.bin".to_owned() } else { cleaned }
}
