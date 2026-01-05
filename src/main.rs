use std::str::FromStr;

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
    // TODO: remove temporary 10 first movies limit
    let movies = result
        .unwrap()
        .into_iter()
        .filter(|m| m.movie_file.as_ref().is_some())
        .take(50)
        .collect::<Vec<_>>();

    // Step 2. Validate or propose scene names
    for movie in movies {
        let scene_name = movie.movie_file.as_deref()
            .and_then(|mf| mf.scene_name.clone().flatten())
            .unwrap_or_else(|| "Unknown".to_string());
        let title: String = movie.title.flatten().unwrap_or_else(|| "Unknown".to_string());
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

        // MediaInfo integration: populate technical info using translated path (fallback to raw)
        let raw_path: Option<String> = movie.movie_file.as_deref()
            .and_then(|mf| mf.path.clone().flatten());
        let tech = match raw_path.as_ref() {
            Some(p) => {
                let local_path = core::media::translate_radarr_path(p, &config);
                tracing::debug!("mediainfo path: radarr='{}' local='{}'", p, local_path.display());
                core::media::mediainfo::collect_technical_info(local_path.to_string_lossy().as_ref())
            }
            None => core::naming::TechnicalInfo::default(),
        };

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

        println!("Title: {} | Year: {:?}", title, hints.year);
        println!("  Path: {}", raw_path.as_deref().unwrap_or("<none>"));
        println!(
            "  Tech: res={:?} vcodec={:?} bitdepth={:?} hdr={} dv={} acodec={:?} ach={:?}",
            tech.resolution, tech.video_codec, tech.bit_depth, tech.hdr, tech.dv, tech.audio_codec, tech.audio_channels
        );
        println!(
            "  Original: {}\n  Proposed: {}\n  Reason: {:?}\n",
            scene_name,
            decision.chosen,
            decision.reason
        );
    }

}