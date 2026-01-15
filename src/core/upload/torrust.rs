use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use tracing::{debug, info};

use crate::utils::Error;

use crate::config::TorrustUploadConfig;

use super::{TrackerUploader, UploadRequest};

fn category_or_default(cfg: &TorrustUploadConfig) -> String {
    cfg.movies_category.clone().unwrap_or_else(|| "movies".to_string())
}

fn tags_json_or_default(cfg: &TorrustUploadConfig) -> String {
    match &cfg.tags {
        Some(tags) => serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()),
        None => "[]".to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct TorrustUploader {
    cfg: TorrustUploadConfig,
    client: reqwest::Client,
}

impl TorrustUploader {
    pub fn new(cfg: TorrustUploadConfig) -> Self {
        Self {
            cfg,
            client: reqwest::Client::new(),
        }
    }

    fn upload_url(&self) -> String {
        format!("{}/torrent/upload", self.cfg.api_base.trim_end_matches('/'))
    }
}

#[async_trait]
impl TrackerUploader for TorrustUploader {
    async fn upload_torrent(&self, req: UploadRequest) -> Result<(), Error> {
        let title_for_logs = req.title.clone();

        let url = self.upload_url();
        let category = category_or_default(&self.cfg);
        let tags_json = tags_json_or_default(&self.cfg);

        info!(target: "seedarr::upload", url = %url, title = %req.title, category = %category, "Uploading torrent to Torrust");

        let torrent_file_name = req
            .torrent_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("upload.torrent")
            .to_string();

        let torrent_bytes = tokio::fs::read(&req.torrent_path)
            .await
            .map_err(|e| Error::Other(format!("Failed to read torrent file '{}': {e}", req.torrent_path.display())))?;

        let torrent_part = Part::bytes(torrent_bytes)
            .file_name(torrent_file_name)
            .mime_str("application/x-bittorrent")
            .map_err(|e| Error::Other(format!("Failed to set torrent mime: {e}")))?;

        let form = Form::new()
            .text("title", req.title)
            .text("description", req.description_markdown)
            .text("category", category)
            .text("tags", tags_json)
            .part("torrent", torrent_part);

        let resp = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .header("Authorization", format!("ApiKey {}", self.cfg.api_key))
            .multipart(form)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        debug!(target: "seedarr::upload", status = %status, body = %body, "Torrust upload response");

        // Idempotency: if the tracker reports the infohash already exists, treat as success.
        if status.as_u16() == 409 {
            let body_lc = body.to_ascii_lowercase();
            if body_lc.contains("infohash") && body_lc.contains("already exists") {
                info!(target: "seedarr::upload", title = %title_for_logs, "Torrent already exists on tracker (409), skipping");
                return Ok(());
            }
        }

        if !status.is_success() {
            return Err(Error::Other(format!("Torrust upload failed: HTTP {status} body={body}")));
        }

        Ok(())
    }
}
