pub mod description;
pub mod torrust;

use async_trait::async_trait;
use std::path::PathBuf;

use crate::utils::Error;

use crate::core::naming::TechnicalInfo;

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub title: String,
    pub description_markdown: String,
    pub torrent_path: PathBuf,
}

#[async_trait]
pub trait TrackerUploader: Send + Sync {
    async fn upload_torrent(&self, req: UploadRequest) -> Result<(), Error>;
}

pub struct UploadService {
    enabled: bool,
    dry_run: bool,
    uploaders: Vec<Box<dyn TrackerUploader>>,
}

impl UploadService {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            dry_run: false,
            uploaders: Vec::new(),
        }
    }

    pub fn from_config(config: &crate::config::Config) -> Result<Self, Error> {
        let Some(upload_cfg) = &config.upload else {
            return Ok(Self::disabled());
        };

        let mut uploaders: Vec<Box<dyn TrackerUploader>> = Vec::new();

        // Preferred: enable each tracker explicitly.
        if let Some(tcfg) = upload_cfg.torrust.as_ref() {
            if tcfg.enable {
                uploaders.push(Box::new(torrust::TorrustUploader::new(tcfg.clone())));
            }
        }

        // Backward compatibility: if `upload.tracker = "torrust"` is set, enable it.
        if uploaders.is_empty() {
            if let Some(tracker) = upload_cfg.tracker.as_ref() {
                if tracker.eq_ignore_ascii_case("torrust") {
                    let Some(tcfg) = upload_cfg.torrust.as_ref() else {
                        return Err(Error::Other(
                            "upload.tracker is 'torrust' but [upload.torrust] config is missing"
                                .to_string(),
                        ));
                    };
                    uploaders.push(Box::new(torrust::TorrustUploader::new(tcfg.clone())));
                } else {
                    return Err(Error::Other(format!(
                        "Unsupported upload.tracker: {tracker}"
                    )));
                }
            }
        }

        let enabled = !uploaders.is_empty();
        Ok(Self {
            enabled,
            dry_run: upload_cfg.dry_run,
            uploaders,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.uploaders.is_empty() && !self.dry_run
    }

    pub async fn upload_movie_torrent(
        &self,
        title: &str,
        year: Option<u16>,
        cover_url: Option<&str>,
        overview: Option<&str>,
        scene_name: &str,
        tech: &TechnicalInfo,
        torrent_path: PathBuf,
    ) -> Result<(), Error> {
        if !self.enabled {
            return Ok(());
        }
        if self.dry_run {
            tracing::info!(
                "Upload dry-run enabled: skipping upload for '{}'",
                scene_name
            );
            return Ok(());
        }

        let md = description::build_markdown(title, year, cover_url, overview, scene_name, tech);
        let req = UploadRequest {
            title: scene_name.to_string(),
            description_markdown: md,
            torrent_path,
        };

        let mut last_err: Option<Error> = None;
        for uploader in &self.uploaders {
            if let Err(e) = uploader.upload_torrent(req.clone()).await {
                tracing::error!("Uploader failed for '{}': {e}", scene_name);
                last_err = Some(e);
            }
        }

        if let Some(e) = last_err {
            Err(e)
        } else {
            Ok(())
        }
    }
}
