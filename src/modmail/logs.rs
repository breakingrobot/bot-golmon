//! Stockage des logs de ticket (`logStorage` : local, attachment, none).

use super::{
    Modmail,
    config::LogStorage,
    formatters::{LogOptions, format_log},
    models::Thread,
    store,
};
use anyhow::Result;
use serde_json::json;

pub struct LogFile {
    pub name: String,
    pub content: String,
}

fn storage_name(storage: &LogStorage) -> &'static str {
    match storage {
        LogStorage::Local => "local",
        LogStorage::Attachment => "attachment",
        LogStorage::None => "none",
    }
}

impl Modmail {
    /// Enregistre le log selon `logStorage` ; le type retenu est mémorisé sur le
    /// ticket pour retrouver le log plus tard, même si la configuration change.
    pub async fn save_log_to_storage(&self, thread: &Thread) -> Result<()> {
        match self.config.log_storage {
            LogStorage::Local | LogStorage::None => {
                store::update_log_storage(&self.db, &thread.id, storage_name(&self.config.log_storage), None).await
            }
            LogStorage::Attachment => {
                // Le fichier n'est écrit qu'à la fermeture, quand le log est complet.
                let Some(current) = store::find_by_id(&self.db, &thread.id).await? else {
                    return Ok(());
                };
                if !current.is_closed() {
                    return Ok(());
                }
                let messages = store::messages(&self.db, &thread.id).await?;
                let filename = format!("{}.txt", thread.id);
                let full_path = self.config.log_attachment_directory.join(&filename);
                tokio::fs::create_dir_all(&self.config.log_attachment_directory).await?;
                tokio::fs::write(&full_path, format_log(&current, &messages, LogOptions::default())).await?;
                let data = json!({ "fullPath": full_path.to_string_lossy(), "filename": filename });
                store::update_log_storage(&self.db, &thread.id, "attachment", Some(&data)).await
            }
        }
    }

    async fn ensure_log_saved(&self, thread: &Thread) -> Result<Thread> {
        if thread.log_storage_type.is_none() {
            self.save_log_to_storage(thread).await?;
            if let Some(updated) = store::find_by_id(&self.db, &thread.id).await? {
                return Ok(updated);
            }
        }
        Ok(thread.clone())
    }

    pub async fn log_url(&self, thread: &Thread) -> Result<Option<String>> {
        let thread = self.ensure_log_saved(thread).await?;
        Ok(match thread.log_storage_type.as_deref() {
            Some("local") => Some(self.self_url(&format!("logs/{}", thread.id)).await),
            _ => None,
        })
    }

    pub async fn log_file(&self, thread: &Thread) -> Result<Option<LogFile>> {
        let thread = self.ensure_log_saved(thread).await?;
        if thread.log_storage_type.as_deref() != Some("attachment") {
            return Ok(None);
        }
        let data: serde_json::Value =
            thread.log_storage_data.as_deref().and_then(|raw| serde_json::from_str(raw).ok()).unwrap_or_default();
        let (Some(path), Some(name)) =
            (data.get("fullPath").and_then(|v| v.as_str()), data.get("filename").and_then(|v| v.as_str()))
        else {
            return Ok(None);
        };
        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(Some(LogFile { name: name.to_owned(), content })),
            Err(_) => Ok(None),
        }
    }

    /// URL publique du serveur web (`url`, sinon IP publique et port).
    pub async fn self_url(&self, path: &str) -> String {
        if let Some(url) = &self.config.url {
            return format!("{url}/{path}");
        }
        let ip = self
            .public_ip
            .get_or_init(|| async {
                match self.http.get("https://api.ipify.org").timeout(std::time::Duration::from_secs(10)).send().await {
                    Ok(response) => response.text().await.unwrap_or_else(|_| "UNKNOWN".to_owned()),
                    Err(error) => {
                        tracing::warn!(%error, "IP publique introuvable");
                        "UNKNOWN".to_owned()
                    }
                }
            })
            .await;
        let host = if ip.contains(':') { format!("[{ip}]") } else { ip.clone() };
        format!("http://{host}:{}/{path}", self.config.port)
    }
}
