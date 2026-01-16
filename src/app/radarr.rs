use std::path::PathBuf;

use crate::core;

pub fn build_radarr_config(
    config: &crate::config::Config,
) -> radarr::apis::configuration::Configuration {
    radarr::apis::configuration::Configuration {
        base_path: config.radarr.base_url.clone(),
        api_key: Some(radarr::apis::configuration::ApiKey {
            prefix: None,
            key: config.radarr.api_key.clone(),
        }),
        ..Default::default()
    }
}

pub async fn fetch_radarr_movies(
    radarr_config: &radarr::apis::configuration::Configuration,
    test_mode: bool,
) -> Result<
    Vec<radarr::models::MovieResource>,
    radarr::apis::Error<radarr::apis::movie_api::ListMovieError>,
> {
    let iter = radarr::apis::movie_api::list_movie(radarr_config, None, None, None)
        .await?
        .into_iter()
        .filter(|m| m.movie_file.as_ref().is_some());

    let movies = if test_mode {
        iter.take(50).collect::<Vec<_>>()
    } else {
        iter.collect::<Vec<_>>()
    };
    Ok(movies)
}

pub async fn process_movie(
    movie: radarr::models::MovieResource,
    config: &crate::config::Config,
    upload_service: &core::upload::UploadService,
) {
    // Step 1. Validate or propose scene names
    let scene_name = extract_scene_name(&movie);
    let title = choose_title(&movie, config);
    let year = movie.year;
    let quality = extract_quality_name(&movie);
    let release_group = extract_release_group(&movie);

    // Build hints for naming
    let hints = core::naming::RadarrHints {
        title: title.clone(),
        year: year.and_then(|y| u16::try_from(y).ok()),
        quality: quality.clone(),
        release_group: release_group.clone(),
    };

    // MediaInfo integration: only process files that are path-mapped in config
    let (raw_path, local_path) = match translate_movie_path(&movie, config) {
        Some(v) => v,
        None => return,
    };

    let mut tech = collect_technical_info(&raw_path, &local_path, config);
    crate::app::common::apply_resolution_fallback(&mut tech, quality.as_deref());

    let cover_url = pick_cover_url(&movie);

    let validation = core::naming::validate_scene_name(&scene_name);
    let decision =
        core::naming::propose_scene_name(Some(&scene_name), &hints, &tech, Some(&validation));

    // Optionally append "-NoTag" if no release group and config requests it
    let mut final_scene_name = decision.chosen.clone();
    if config.media.append_no_tag_on_missing_group && release_group.is_none() {
        final_scene_name.push_str("-NoTag");
    }

    println!("Title: {} | Year: {:?}", title, hints.year);
    println!("  Path: {}", raw_path);
    println!(
        "  Tech: res={:?} vcodec={:?} bitdepth={:?} hdr={} dv={} acodec={:?} ach={:?}",
        tech.resolution,
        tech.video_codec,
        tech.bit_depth,
        tech.hdr,
        tech.dv,
        tech.audio_codec,
        tech.audio_channels
    );
    println!(
        "  Original: {}\n  Proposed: {}\n  Reason: {:?}\n",
        scene_name, final_scene_name, decision.reason
    );

    // Step 2. Create seed symlink structure if configured
    if let Some(seed_root) = &config.media.seed_path {
        let src_video = local_path.as_path();
        if let Err(e) = core::fs::export_seed_structure(
            PathBuf::from(seed_root).as_path(),
            &final_scene_name,
            src_video,
        ) {
            tracing::error!(
                "Failed to export seed structure for '{}': {}",
                decision.chosen,
                e
            );
        }

        // Step 3. Create .torrent for the seeded scene directory via intermodal (unless dry_run)
        if config.torrent.dry_run {
            tracing::info!(
                "Dry-run enabled: skipping torrent creation for '{}'",
                final_scene_name
            );
            return;
        }

        let seed_dir = PathBuf::from(seed_root).join(&final_scene_name);
        match core::torrent::create_torrent_for_seed_dir(
            seed_dir.as_path(),
            &final_scene_name,
            config,
        ) {
            Ok(torrent_path) => {
                // Step 4. Upload torrent to private trackers (optional)
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
                    tracing::info!(
                        "Upload service disabled: skipping upload for '{}'",
                        final_scene_name
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to create torrent for '{}': {}", decision.chosen, e);
            }
        }
    }
}

fn extract_scene_name(movie: &radarr::models::MovieResource) -> String {
    movie
        .movie_file
        .as_deref()
        .and_then(|mf| mf.scene_name.clone().flatten())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn choose_title(movie: &radarr::models::MovieResource, config: &crate::config::Config) -> String {
    let original_title = movie
        .original_title
        .clone()
        .flatten()
        .unwrap_or_else(|| "Unknown".to_string());
    let local_title = movie
        .title
        .clone()
        .flatten()
        .unwrap_or_else(|| "Unknown".to_string());

    if let Some(strategy) = &config.media.title_strategy {
        match strategy {
            crate::config::TitleStrategy::OriginalIfEnElseLocal => {
                let is_en = movie
                    .original_language
                    .as_deref()
                    .and_then(|ol| ol.name.clone().flatten())
                    .map(|l| {
                        let ll = l.to_ascii_lowercase();
                        ll.starts_with("en") || ll.contains("english")
                    })
                    .unwrap_or(false);

                if is_en {
                    original_title
                } else {
                    local_title
                }
            }
            crate::config::TitleStrategy::AlwaysLocal => local_title,
        }
    } else if config.media.use_original_title {
        original_title
    } else {
        local_title
    }
}

fn extract_quality_name(movie: &radarr::models::MovieResource) -> Option<String> {
    movie
        .movie_file
        .as_deref()
        .and_then(|mf| {
            mf.quality.as_deref().and_then(|q| {
                q.quality
                    .as_deref()
                    .and_then(|q2| q2.name.as_ref().cloned())
            })
        })
        .flatten()
}

fn extract_release_group(movie: &radarr::models::MovieResource) -> Option<String> {
    movie
        .movie_file
        .as_deref()
        .and_then(|mf| mf.release_group.clone().flatten())
}

fn translate_movie_path(
    movie: &radarr::models::MovieResource,
    config: &crate::config::Config,
) -> Option<(String, PathBuf)> {
    let raw_path = movie
        .movie_file
        .as_deref()
        .and_then(|mf| mf.path.clone().flatten());

    let raw_path = match raw_path {
        Some(p) => p,
        None => {
            tracing::warn!("Skipping movie with no file path");
            return None;
        }
    };

    match core::media::try_translate_radarr_path(&raw_path, config) {
        Some(lp) => Some((raw_path, lp)),
        None => {
            tracing::warn!(
                "Skipping unmapped path (no radarr.path_mappings match): {}",
                raw_path
            );
            None
        }
    }
}

fn collect_technical_info(
    raw_path: &str,
    local_path: &PathBuf,
    config: &crate::config::Config,
) -> core::naming::TechnicalInfo {
    tracing::debug!(
        "mediainfo path: radarr='{}' local='{}'",
        raw_path,
        local_path.display()
    );
    core::media::mediainfo::collect_technical_info_with_cache(
        local_path.to_string_lossy().as_ref(),
        config.media.enable_mediainfo_cache,
    )
}

fn pick_cover_url(movie: &radarr::models::MovieResource) -> Option<String> {
    // Prefer images[*].remoteUrl, fallback to images[*].url, then remotePoster
    movie
        .images
        .as_ref()
        .and_then(|v| v.as_ref())
        .and_then(|imgs| {
            imgs.iter().find_map(|img| {
                img.remote_url
                    .clone()
                    .flatten()
                    .or_else(|| img.url.clone().flatten())
            })
        })
        .or_else(|| movie.remote_poster.clone().flatten())
}
