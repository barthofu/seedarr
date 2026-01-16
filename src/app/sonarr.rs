use std::path::PathBuf;

use crate::core;

pub async fn run_sonarr_pipeline(
    config: &crate::config::Config,
    upload_service: &core::upload::UploadService,
) -> Result<(), crate::utils::Error> {
    let Some(sonarr_cfg) = &config.sonarr else {
        tracing::info!("Sonarr not configured: skipping series pipeline");
        return Ok(());
    };

    tracing::info!(
        "Sonarr pipeline starting (test_mode={}, only_complete_seasons={}, integrale_if_complete={}, per_episode_for_incomplete_seasons={})",
        config.test_mode,
        sonarr_cfg.only_complete_seasons,
        sonarr_cfg.create_integrale_pack_if_complete,
        sonarr_cfg.per_episode_for_incomplete_seasons
    );

    let client =
        core::sonarr::SonarrClient::new(sonarr_cfg.base_url.clone(), sonarr_cfg.api_key.clone());

    let series_list = client.list_series().await?;
    tracing::info!("Fetched {} series from Sonarr", series_list.len());
    let series_iter = series_list.into_iter();
    let series_iter: Box<dyn Iterator<Item = core::sonarr::SeriesResource>> = if config.test_mode {
        Box::new(series_iter.take(10))
    } else {
        Box::new(series_iter)
    };

    for series in series_iter {
        tracing::info!(
            "Processing series: '{}' (id={})",
            series.title,
            series.id
        );
        let kind = content_kind_from_series_type(series.series_type.as_deref());
        let cover_url = pick_sonarr_cover_url(&series);

        let episodes = client.list_episodes(series.id).await?;
        let mut episode_by_id: std::collections::HashMap<i64, core::sonarr::EpisodeResource> =
            std::collections::HashMap::new();
        let mut episodes_by_season: std::collections::HashMap<u16, Vec<i64>> =
            std::collections::HashMap::new();

        for ep in episodes {
            // Ignore specials/season 0 by default (not requested for packs)
            if ep.season_number <= 0 {
                episode_by_id.insert(ep.id, ep);
                continue;
            }
            let season_u = u16::try_from(ep.season_number).ok();
            if let Some(s) = season_u {
                episodes_by_season.entry(s).or_default().push(ep.id);
            }
            episode_by_id.insert(ep.id, ep);
        }

        let episode_files = client.list_episode_files(series.id).await?;
        tracing::info!(
            "Sonarr series '{}' has {} episodes and {} episode files",
            series.title,
            episode_by_id.len(),
            episode_files.len()
        );

        let mut season_to_files: std::collections::HashMap<
            u16,
            Vec<core::sonarr::EpisodeFileResource>,
        > = std::collections::HashMap::new();
        let mut all_mapped_files: Vec<core::sonarr::EpisodeFileResource> = Vec::new();

        let mut unmapped_episode_files: usize = 0;
        for epf in episode_files {
            // Skip episode files we can't map locally (strict behavior)
            if translate_episode_path(&epf, config).is_none() {
                unmapped_episode_files += 1;
                continue;
            }
            let mut seasons_in_file: std::collections::BTreeSet<u16> =
                std::collections::BTreeSet::new();

            // Prefer Sonarr's seasonNumber if present; it's the most reliable signal for
            // season-based packs (even for multi-episode files).
            if let Some(sn) = epf.season_number {
                if sn > 0 {
                    if let Ok(s) = u16::try_from(sn) {
                        seasons_in_file.insert(s);
                    }
                }
            }

            // Fallback: infer season(s) from episodeIds.
            for eid in &epf.episode_ids {
                if let Some(ep) = episode_by_id.get(eid) {
                    if ep.season_number > 0 {
                        if let Ok(s) = u16::try_from(ep.season_number) {
                            seasons_in_file.insert(s);
                        }
                    }
                }
            }

            // Only include episode files that belong to exactly one season in season packs.
            if seasons_in_file.len() == 1 {
                if let Some(season) = seasons_in_file.into_iter().next() {
                    season_to_files.entry(season).or_default().push(epf.clone());
                }
            }
            all_mapped_files.push(epf);
        }

        if unmapped_episode_files > 0 {
            tracing::warn!(
                "Series '{}' skipped {} episode files due to missing sonarr.path_mappings",
                series.title,
                unmapped_episode_files
            );
        }

        // Determine season completeness
        let mut complete_seasons: std::collections::BTreeSet<u16> =
            std::collections::BTreeSet::new();
        let mut incomplete_seasons: std::collections::BTreeSet<u16> =
            std::collections::BTreeSet::new();

        for (season, ep_ids) in &episodes_by_season {
            if ep_ids.is_empty() {
                continue;
            }

            // Use Sonarr's own view of completion (hasFile/monitored) rather than inferring from
            // episodefile <-> episodeIds matching (which can be skewed by mapping/skips).
            let is_complete = ep_ids.iter().all(|eid| {
                episode_by_id
                    .get(eid)
                    .map(|ep| !ep.monitored || ep.has_file)
                    .unwrap_or(false)
            });
            if is_complete {
                complete_seasons.insert(*season);
            } else {
                incomplete_seasons.insert(*season);
            }
        }

        let series_complete = incomplete_seasons.is_empty() && !episodes_by_season.is_empty();

        tracing::info!(
            "Series '{}' season status: complete={} incomplete={} (series_complete={})",
            series.title,
            complete_seasons.len(),
            incomplete_seasons.len(),
            series_complete
        );

        // Default behavior: create season packs only when the season is complete.
        // If only_complete_seasons=false, create season packs for all seasons with any episodes.
        let pack_seasons: std::collections::BTreeSet<u16> = if sonarr_cfg.only_complete_seasons {
            complete_seasons.clone()
        } else {
            complete_seasons
                .union(&incomplete_seasons)
                .copied()
                .collect()
        };

        if pack_seasons.is_empty() {
            tracing::info!(
                "Series '{}' has no seasons eligible for season packs (only_complete_seasons={})",
                series.title,
                sonarr_cfg.only_complete_seasons
            );
        }

        for season in pack_seasons.iter().copied() {
            if let Some(files) = season_to_files.get(&season) {
                tracing::info!(
                    "Creating season pack for '{}' season S{:02} using {} episode files",
                    series.title,
                    season,
                    files.len()
                );
                create_season_pack(
                    &series,
                    season,
                    files,
                    cover_url.as_deref(),
                    kind,
                    config,
                    upload_service,
                )
                .await;
            } else {
                tracing::warn!(
                    "Skipping season pack for '{}' season S{:02}: no eligible episode files (files may be unmapped, missing episodeIds, or span multiple seasons)",
                    series.title,
                    season
                );
            }
        }

        // Optional: create integrale pack if the entire series is complete.
        if sonarr_cfg.create_integrale_pack_if_complete && series_complete {
            tracing::info!("Creating INTEGRALE pack for '{}'", series.title);
            create_integrale_pack(
                &series,
                &all_mapped_files,
                cover_url.as_deref(),
                kind,
                config,
                upload_service,
            )
            .await;
        } else if sonarr_cfg.create_integrale_pack_if_complete {
            tracing::info!(
                "INTEGRALE pack requested but '{}' is not complete; skipping",
                series.title
            );
        }

        // Optional: per-episode torrents only for seasons that are not complete.
        if sonarr_cfg.per_episode_for_incomplete_seasons {
            for season in incomplete_seasons.iter().copied() {
                // If we're creating a season pack for this season, don't also emit per-episode torrents.
                if pack_seasons.contains(&season) {
                    continue;
                }
                if let Some(files) = season_to_files.get(&season) {
                    // Deduplicate by path (episode files can be pushed multiple times via multiple episodeIds)
                    let mut seen_paths: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    for epf in files {
                        if !seen_paths.insert(epf.path.clone()) {
                            continue;
                        }
                        tracing::info!(
                            "Creating per-episode torrent for '{}' season S{:02}: {}",
                            series.title,
                            season,
                            epf.path
                        );
                        process_episode_file(
                            &series,
                            epf,
                            &episode_by_id,
                            cover_url.as_deref(),
                            kind,
                            config,
                            upload_service,
                        )
                        .await;
                    }
                }
            }
        } else if !incomplete_seasons.is_empty() {
            tracing::info!(
                "Series '{}' has incomplete seasons but per-episode fallback is disabled; nothing will be generated for those seasons",
                series.title
            );
        }
    }

    tracing::info!("Sonarr pipeline finished");
    Ok(())
}

async fn create_season_pack(
    series: &core::sonarr::SeriesResource,
    season: u16,
    episode_files: &[core::sonarr::EpisodeFileResource],
    cover_url: Option<&str>,
    kind: core::upload::ContentKind,
    config: &crate::config::Config,
    upload_service: &core::upload::UploadService,
) {
    let Some(seed_root) = &config.media.seed_path else {
        return;
    };

    let mut unique_paths: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    let mut qualities: Vec<String> = Vec::new();
    let mut release_groups: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for epf in episode_files {
        if let Some(lp) = translate_episode_path(epf, config) {
            unique_paths.insert(lp);
        }
        if let Some(q) = extract_sonarr_quality_name(epf) {
            qualities.push(q);
        }
        if let Some(rg) = epf.release_group.as_ref() {
            release_groups.insert(rg.clone());
        }
    }

    let src_videos: Vec<PathBuf> = unique_paths.into_iter().collect();
    if src_videos.is_empty() {
        return;
    }

    let mut tech = core::media::mediainfo::collect_technical_info_with_cache(
        src_videos[0].to_string_lossy().as_ref(),
        config.media.enable_mediainfo_cache,
    );

    let quality = qualities.into_iter().next();
    crate::app::common::apply_resolution_fallback(&mut tech, quality.as_deref());

    let release_group = if release_groups.len() == 1 {
        release_groups.into_iter().next()
    } else {
        None
    };

    let hints = core::naming::PackHints {
        title: series.title.clone(),
        year: series.year.and_then(|y| u16::try_from(y).ok()),
        pack_tag: format!("S{:02}", season),
        quality,
        release_group,
    };
    let decision = core::naming::propose_pack_scene_name(None, &hints, &tech);

    let mut final_scene_name = decision.chosen.clone();
    if config.media.append_no_tag_on_missing_group && hints.release_group.is_none() {
        final_scene_name.push_str("-NoTag");
    }

    if let Err(e) = core::fs::export_seed_pack_structure(
        PathBuf::from(seed_root).as_path(),
        &final_scene_name,
        &src_videos,
    ) {
        tracing::error!("Failed to export season pack '{}': {}", final_scene_name, e);
        return;
    }

    if config.torrent.dry_run {
        tracing::info!(
            "Dry-run enabled: skipping torrent creation for '{}'",
            final_scene_name
        );
        return;
    }

    let seed_dir = PathBuf::from(seed_root).join(&final_scene_name);
    match core::torrent::create_torrent_for_seed_dir(seed_dir.as_path(), &final_scene_name, config)
    {
        Ok(torrent_path) => {
            let heading = format!("S{:02} Complete", season);
            let overview = series.overview.as_deref();
            if let Err(e) = upload_service
                .upload_episode_torrent(
                    &series.title,
                    &heading,
                    cover_url,
                    overview,
                    &final_scene_name,
                    &tech,
                    torrent_path,
                    kind,
                )
                .await
            {
                tracing::error!("Failed to upload torrent for '{}': {e}", final_scene_name);
            }
        }
        Err(e) => {
            tracing::error!("Failed to create torrent for '{}': {}", final_scene_name, e);
        }
    }
}

async fn create_integrale_pack(
    series: &core::sonarr::SeriesResource,
    episode_files: &[core::sonarr::EpisodeFileResource],
    cover_url: Option<&str>,
    kind: core::upload::ContentKind,
    config: &crate::config::Config,
    upload_service: &core::upload::UploadService,
) {
    let Some(seed_root) = &config.media.seed_path else {
        return;
    };

    let mut unique_paths: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    let mut qualities: Vec<String> = Vec::new();
    let mut release_groups: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for epf in episode_files {
        if let Some(lp) = translate_episode_path(epf, config) {
            unique_paths.insert(lp);
        }
        if let Some(q) = extract_sonarr_quality_name(epf) {
            qualities.push(q);
        }
        if let Some(rg) = epf.release_group.as_ref() {
            release_groups.insert(rg.clone());
        }
    }

    let src_videos: Vec<PathBuf> = unique_paths.into_iter().collect();
    if src_videos.is_empty() {
        return;
    }

    let mut tech = core::media::mediainfo::collect_technical_info_with_cache(
        src_videos[0].to_string_lossy().as_ref(),
        config.media.enable_mediainfo_cache,
    );

    let quality = qualities.into_iter().next();
    crate::app::common::apply_resolution_fallback(&mut tech, quality.as_deref());

    let release_group = if release_groups.len() == 1 {
        release_groups.into_iter().next()
    } else {
        None
    };

    let hints = core::naming::PackHints {
        title: series.title.clone(),
        year: series.year.and_then(|y| u16::try_from(y).ok()),
        pack_tag: "INTEGRALE".to_string(),
        quality,
        release_group,
    };
    let decision = core::naming::propose_pack_scene_name(None, &hints, &tech);

    let mut final_scene_name = decision.chosen.clone();
    if config.media.append_no_tag_on_missing_group && hints.release_group.is_none() {
        final_scene_name.push_str("-NoTag");
    }

    if let Err(e) = core::fs::export_seed_pack_structure(
        PathBuf::from(seed_root).as_path(),
        &final_scene_name,
        &src_videos,
    ) {
        tracing::error!(
            "Failed to export integrale pack '{}': {}",
            final_scene_name,
            e
        );
        return;
    }

    if config.torrent.dry_run {
        tracing::info!(
            "Dry-run enabled: skipping torrent creation for '{}'",
            final_scene_name
        );
        return;
    }

    let seed_dir = PathBuf::from(seed_root).join(&final_scene_name);
    match core::torrent::create_torrent_for_seed_dir(seed_dir.as_path(), &final_scene_name, config)
    {
        Ok(torrent_path) => {
            let heading = "Integrale".to_string();
            let overview = series.overview.as_deref();
            if let Err(e) = upload_service
                .upload_episode_torrent(
                    &series.title,
                    &heading,
                    cover_url,
                    overview,
                    &final_scene_name,
                    &tech,
                    torrent_path,
                    kind,
                )
                .await
            {
                tracing::error!("Failed to upload torrent for '{}': {e}", final_scene_name);
            }
        }
        Err(e) => {
            tracing::error!("Failed to create torrent for '{}': {}", final_scene_name, e);
        }
    }
}

fn content_kind_from_series_type(series_type: Option<&str>) -> core::upload::ContentKind {
    let Some(st) = series_type else {
        return core::upload::ContentKind::Series;
    };
    let l = st.to_ascii_lowercase();
    if l.contains("anime") {
        core::upload::ContentKind::Anime
    } else {
        core::upload::ContentKind::Series
    }
}

fn pick_sonarr_cover_url(series: &core::sonarr::SeriesResource) -> Option<String> {
    series
        .images
        .iter()
        .find_map(|img| img.remote_url.clone().or_else(|| img.url.clone()))
}

fn extract_sonarr_quality_name(epf: &core::sonarr::EpisodeFileResource) -> Option<String> {
    epf.quality
        .as_ref()
        .and_then(|q| q.quality.as_ref())
        .and_then(|q2| q2.name.clone())
}

fn translate_episode_path(
    epf: &core::sonarr::EpisodeFileResource,
    config: &crate::config::Config,
) -> Option<PathBuf> {
    match core::media::try_translate_sonarr_path(&epf.path, config) {
        Some(p) => Some(p),
        None => {
            tracing::warn!(
                "Skipping unmapped path (no sonarr.path_mappings match): {}",
                epf.path
            );
            None
        }
    }
}

fn format_episode_heading(
    season_number: Option<u16>,
    episode_numbers: &[u16],
    absolute_numbers: &[u16],
    episode_title: Option<&str>,
) -> String {
    let mut tag = String::new();

    if !absolute_numbers.is_empty() {
        let mut abs = absolute_numbers.to_vec();
        abs.sort_unstable();
        abs.dedup();
        for n in abs {
            tag.push_str(&format!("E{:03}", n));
        }
    } else if let Some(season) = season_number {
        tag.push_str(&format!("S{:02}", season));
        let mut eps = episode_numbers.to_vec();
        eps.sort_unstable();
        eps.dedup();
        for e in eps {
            tag.push_str(&format!("E{:02}", e));
        }
    }

    if let Some(t) = episode_title {
        let t = t.trim();
        if !t.is_empty() {
            if tag.is_empty() {
                return t.to_string();
            }
            return format!("{tag} — {t}");
        }
    }

    tag
}

async fn process_episode_file(
    series: &core::sonarr::SeriesResource,
    epf: &core::sonarr::EpisodeFileResource,
    episode_by_id: &std::collections::HashMap<i64, core::sonarr::EpisodeResource>,
    cover_url: Option<&str>,
    kind: core::upload::ContentKind,
    config: &crate::config::Config,
    upload_service: &core::upload::UploadService,
) {
    if epf.episode_ids.is_empty() {
        tracing::warn!(
            "Skipping episode file with no episodeIds: path={} ",
            epf.path
        );
        return;
    }

    let local_path = match translate_episode_path(epf, config) {
        Some(p) => p,
        None => return,
    };

    tracing::debug!(
        "mediainfo path: sonarr='{}' local='{}'",
        epf.path,
        local_path.display()
    );
    let mut tech = core::media::mediainfo::collect_technical_info_with_cache(
        local_path.to_string_lossy().as_ref(),
        config.media.enable_mediainfo_cache,
    );

    let quality = extract_sonarr_quality_name(epf);
    crate::app::common::apply_resolution_fallback(&mut tech, quality.as_deref());

    // Episode metadata (may be multi-episode)
    let mut season_number: Option<u16> = None;
    let mut episode_numbers: Vec<u16> = Vec::new();
    let mut absolute_numbers: Vec<u16> = Vec::new();
    let mut overview: Option<String> = None;
    let mut episode_title: Option<String> = None;

    for eid in &epf.episode_ids {
        if let Some(ep) = episode_by_id.get(eid) {
            if season_number.is_none() {
                season_number = u16::try_from(ep.season_number).ok();
            }
            if let Ok(n) = u16::try_from(ep.episode_number) {
                episode_numbers.push(n);
            }
            if let Some(abs) = ep.absolute_episode_number {
                if let Ok(n) = u16::try_from(abs) {
                    absolute_numbers.push(n);
                }
            }
            if overview.is_none() {
                overview = ep.overview.clone();
            }
            if episode_title.is_none() {
                episode_title = ep.title.clone();
            }
        }
    }

    let hints = core::naming::EpisodeHints {
        series_title: series.title.clone(),
        series_year: series.year.and_then(|y| u16::try_from(y).ok()),
        season_number,
        episode_numbers: episode_numbers.clone(),
        absolute_episode_numbers: absolute_numbers.clone(),
        quality: quality.clone(),
        release_group: epf.release_group.clone(),
    };

    let original_scene = epf.scene_name.as_deref();
    let decision = core::naming::propose_episode_scene_name(original_scene, &hints, &tech);

    let mut final_scene_name = decision.chosen.clone();
    if config.media.append_no_tag_on_missing_group && epf.release_group.is_none() {
        final_scene_name.push_str("-NoTag");
    }

    let episode_heading = format_episode_heading(
        season_number,
        &episode_numbers,
        &absolute_numbers,
        episode_title.as_deref(),
    );

    println!("Series: {}", series.title);
    println!("  Path: {}", epf.path);
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
        original_scene.unwrap_or("<none>"),
        final_scene_name,
        decision.reason
    );

    // Seed + torrent + upload reuse the same pipeline
    let Some(seed_root) = &config.media.seed_path else {
        return;
    };

    if let Err(e) = core::fs::export_seed_structure(
        PathBuf::from(seed_root).as_path(),
        &final_scene_name,
        local_path.as_path(),
    ) {
        tracing::error!(
            "Failed to export seed structure for '{}': {}",
            final_scene_name,
            e
        );
    }

    if config.torrent.dry_run {
        tracing::info!(
            "Dry-run enabled: skipping torrent creation for '{}'",
            final_scene_name
        );
        return;
    }

    let seed_dir = PathBuf::from(seed_root).join(&final_scene_name);
    match core::torrent::create_torrent_for_seed_dir(seed_dir.as_path(), &final_scene_name, config)
    {
        Ok(torrent_path) => {
            if let Err(e) = upload_service
                .upload_episode_torrent(
                    &series.title,
                    &episode_heading,
                    cover_url,
                    overview.as_deref(),
                    &final_scene_name,
                    &tech,
                    torrent_path,
                    kind,
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
            tracing::error!("Failed to create torrent for '{}': {}", final_scene_name, e);
        }
    }
}
