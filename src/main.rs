mod app;
mod config;
mod core;
mod utils;

#[dotenvy::load(path = "./.env", required = true)]
#[tokio::main]
async fn main() {
    let config = config::Config::init().expect("Failed to initialize configuration");
    app::common::init_logging(&config);
    if let Err(e) = app::common::ensure_seed_path(&config) {
        tracing::error!("{e}");
        return;
    }

    let radarr_config = app::radarr::build_radarr_config(&config);
    let movies = match app::radarr::fetch_radarr_movies(&radarr_config, config.test_mode).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to list movies from Radarr: {e}");
            return;
        }
    };

    // Optional: upload service (private tracker uploads). Keep main tracker-agnostic.
    let upload_service = match core::upload::UploadService::from_config(&config) {
        Ok(svc) => svc,
        Err(e) => {
            tracing::error!("Upload configuration error: {e}");
            core::upload::UploadService::disabled()
        }
    };

    for movie in movies {
        app::radarr::process_movie(movie, &config, &upload_service).await;
    }

    if let Err(e) = app::sonarr::run_sonarr_pipeline(&config, &upload_service).await {
        tracing::error!("Sonarr pipeline failed: {e}");
    }
}
