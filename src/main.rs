use std::str::FromStr;
use std::path::PathBuf;

use tracing::Level;

mod config;
mod utils;
mod core;

#[dotenvy::load(path = "./.env", required = true)]
#[tokio::main]
async fn main() {
    
    // Step 0. Initialization
    let config = config::Config::init().expect("Failed to initialize configuration");
    tracing_subscriber::fmt()
        .with_max_level(
            Level::from_str(&config.logs.level)
                .unwrap_or(Level::INFO)
        )
        .init();

    // Safety: ensure seed_path exists (and is a directory) at startup
    if let Some(seed_root) = &config.media.seed_path {
        let seed_path = PathBuf::from(seed_root);
        if seed_path.exists() {
            if !seed_path.is_dir() {
                tracing::error!("Configured seed_path '{}' exists but is not a directory", seed_path.display());
                return;
            }
        } else {
            if let Err(e) = std::fs::create_dir_all(&seed_path) {
                tracing::error!("Failed to create configured seed_path '{}': {}", seed_path.display(), e);
                return;
            } else {
                tracing::info!("Created seed_path directory: '{}'", seed_path.display());
            }
        }
    }

    // Step 1. List all movies in Radarr
    let radarr_config = radarr::apis::configuration::Configuration {
        base_path: config.radarr.base_url.clone(),
        api_key: Some(radarr::apis::configuration::ApiKey {
            prefix: None,
            key: config.radarr.api_key.clone(),
        }),
        ..Default::default()
    };

    let result = radarr::apis::movie_api::list_movie(&radarr_config, None, None, None).await;
    let movies = result
        .unwrap()
        .into_iter()
        .filter(|m| m.movie_file.as_ref().is_some())
        .collect::<Vec<_>>();

    // Optional: upload service (private tracker uploads). Keep main tracker-agnostic.
    let upload_service = match core::upload::UploadService::from_config(&config) {
        Ok(svc) => svc,
        Err(e) => {
            tracing::error!("Upload configuration error: {e}");
            core::upload::UploadService::disabled()
        }
    };

    // Step 2. Validate or propose scene names
    for movie in movies {
        let scene_name = movie.movie_file.as_deref()
            .and_then(|mf| mf.scene_name.clone().flatten())
            .unwrap_or_else(|| "Unknown".to_string());
        // Title selection
        let original_title = movie.original_title.flatten().unwrap_or_else(|| "Unknown".to_string());
        let local_title = movie.title.flatten().unwrap_or_else(|| "Unknown".to_string());
        let title: String = if let Some(strategy) = &config.media.title_strategy {
            match strategy {
                crate::config::TitleStrategy::OriginalIfEnElseLocal => {
                    // Prefer Radarr movie.original_language when available as string code
                    let is_en = movie
                        .original_language
                        .as_deref()
                        .and_then(|ol| ol.name.clone().flatten())
                        .map(|l| {
                            let ll = l.to_ascii_lowercase();
                            ll.starts_with("en") || ll.contains("english")
                        })
                        .unwrap_or(false);
                    if is_en { original_title } else { local_title }
                }
                crate::config::TitleStrategy::AlwaysLocal => local_title,
            }
        } else {
            if config.media.use_original_title { original_title } else { local_title }
        };
        // let path: Option<String> = movie.movie_file.as_deref()
        //     .and_then(|mf| mf.path.clone().flatten());
        let year: Option<i32> = movie.year;
        let quality: Option<String> = movie.movie_file.as_deref()
            .and_then(|mf| mf.quality.as_deref()
                .and_then(|q| q.quality.as_deref()
                    .and_then(|q2| q2.name.as_ref().cloned())))
            .flatten();
        let release_group: Option<String> = movie.movie_file.as_deref()
            .and_then(|mf| mf.release_group.clone().flatten());

        // Build hints for naming
        let hints = core::naming::RadarrHints {
            title: title.clone(),
            year: year.and_then(|y| u16::try_from(y).ok()),
            quality: quality.clone(),
            release_group: release_group.clone(),
        };

        // MediaInfo integration: only process files that are path-mapped in config
        let raw_path: Option<String> = movie.movie_file.as_deref()
            .and_then(|mf| mf.path.clone().flatten());
        let local_path = match raw_path.as_ref() {
            Some(p) => {
                match core::media::try_translate_radarr_path(p, &config) {
                    Some(lp) => Some(lp),
                    None => {
                        tracing::warn!("Skipping unmapped path (no radarr.path_mappings match): {}", p);
                        continue;
                    }
                }
            }
            None => {
                tracing::warn!("Skipping movie with no file path");
                continue;
            }
        };
        let tech = {
            tracing::debug!("mediainfo path: radarr='{}' local='{}'", raw_path.as_deref().unwrap_or("<none>"), local_path.as_ref().unwrap().display());
            core::media::mediainfo::collect_technical_info_with_cache(
                local_path.as_ref().unwrap().to_string_lossy().as_ref(),
                config.media.enable_mediainfo_cache,
            )
        };

        // Optional: movie cover URL for upload descriptions (prefer images[*].remoteUrl, fallback to images[*].url, then remotePoster)
        let cover_url: Option<String> = movie
            .images
            .as_ref()
            .and_then(|v| v.as_ref())
            .and_then(|imgs| {
                imgs.iter().find_map(|img| {
                    img.remote_url.clone().flatten().or_else(|| img.url.clone().flatten())
                })
            })
            .or_else(|| movie.remote_poster.clone().flatten());

        // Fallback: derive resolution from Radarr quality if MediaInfo didn't provide it
        let mut tech = tech;
        if tech.resolution.is_none() {
            if let Some(q) = &quality {
                let ql = q.to_ascii_lowercase();
                tech.resolution = if ql.contains("2160p") || ql.contains("4k") { Some("2160p".to_string()) }
                    else if ql.contains("1440p") { Some("1440p".to_string()) }
                    else if ql.contains("1080p") { Some("1080p".to_string()) }
                    else if ql.contains("720p") { Some("720p".to_string()) }
                    else { None };
            }
        }

        let validation = core::naming::validate_scene_name(&scene_name);
        let decision = core::naming::propose_scene_name(
            Some(&scene_name),
            &hints,
            &tech,
            Some(&validation),
        );
        // Optionally append "-NoTag" if no release group and config requests it
        let mut final_scene_name = decision.chosen.clone();
        if config.media.append_no_tag_on_missing_group && release_group.is_none() {
            final_scene_name.push_str("-NoTag");
        }

        println!("Title: {} | Year: {:?}", title, hints.year);
        println!("  Path: {}", raw_path.as_deref().unwrap_or("<none>"));
        println!(
            "  Tech: res={:?} vcodec={:?} bitdepth={:?} hdr={} dv={} acodec={:?} ach={:?}",
            tech.resolution, tech.video_codec, tech.bit_depth, tech.hdr, tech.dv, tech.audio_codec, tech.audio_channels
        );
        println!(
            "  Original: {}\n  Proposed: {}\n  Reason: {:?}\n",
            scene_name,
            final_scene_name,
            decision.reason
        );

        // Step 3. Create seed symlink structure if configured
        if let Some(seed_root) = &config.media.seed_path {
            let src_video = local_path.as_ref().unwrap();
            if let Err(e) = core::fs::export_seed_structure(PathBuf::from(seed_root).as_path(), &final_scene_name, src_video.as_path()) {
                tracing::error!("Failed to export seed structure for '{}': {}", decision.chosen, e);
            }
            // Step 4. Create .torrent for the seeded scene directory via intermodal (unless dry_run)
            if !config.torrent.dry_run {
                let seed_dir = PathBuf::from(seed_root).join(&final_scene_name);
                match core::torrent::create_torrent_for_seed_dir(seed_dir.as_path(), &final_scene_name, &config) {
                    Ok(torrent_path) => {
                        // Step 5. Upload torrent to a private tracker (optional)
                        let overview = movie.overview.clone().flatten();
                        if let Err(e) = upload_service
                            .upload_movie_torrent(
                                &title,
                                hints.year,
                                cover_url.as_deref(),
                                overview.as_deref(),
                                &final_scene_name,
                                &tech,
                                torrent_path,
                            )
                            .await
                        {
                            tracing::error!("Failed to upload torrent for '{}': {e}", final_scene_name);
                        } else if upload_service.is_enabled() {
                            tracing::info!("Uploaded torrent for '{}'", final_scene_name);
                        } else {
                            tracing::info!("Upload service disabled: skipping upload for '{}'", final_scene_name);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to create torrent for '{}': {}", decision.chosen, e);
                    }
                }
            } else {
                tracing::info!("Dry-run enabled: skipping torrent creation for '{}'", final_scene_name);
            }
        }
    }

}