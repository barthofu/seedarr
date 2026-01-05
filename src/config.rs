use serde::Deserialize;

impl Config {

    pub fn init() -> Result<Self, config::ConfigError> {
        // get config toml dir from env, with default
        let config_path =
            std::env::var("GHOSTSEED_CONFIG_PATH").unwrap_or_else(|_| String::from("./config.toml"));

        let config = config::Config::builder()
            // Add in config toml
            .add_source(config::File::with_name(&config_path))
            // Add in settings from the environment (with a prefix of GHOSTSEED)
            .add_source(config::Environment::with_prefix("GHOSTSEED").separator("__"))
            .build()?;

        config.try_deserialize()
    }
}

// ================================================================================================
// Models
// ================================================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct Config {
    // pub general: GeneralConfig,
    pub logs: LogsConfig,
    pub radarr: RadarrConfig,
    pub paths: Option<PathsConfig>,
}

// ===============================================================================
// General
// ===============================================================================

// #[derive(Debug, Clone, Deserialize)]
// #[allow(unused)]
// pub struct GeneralConfig {
//     pub base_library_dir: String,
//     pub temp_download_dir: String,
// }

// ===============================================================================
// Logs
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct LogsConfig {
    pub level: String,
    pub enable_reqwest_logging: bool,
}

// ===============================================================================
// Radarr
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct RadarrConfig {
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub path_mappings: Vec<PathMap>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct PathMap {
    /// Prefix as seen by Radarr (e.g. "/data/library/movies")
    pub radarr_root: String,
    /// Local absolute prefix (e.g. "/mnt/nas/medias/plex/library/movies")
    pub local_root: String,
}

// ===============================================================================
// Paths Mapping
// ===============================================================================
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct PathsConfig {
    /// Prefix reported by Radarr inside its container (e.g., "/movies")
    pub radarr_root: String,
    /// Local filesystem root where the same library is mounted (e.g., "/mnt/media/movies")
    pub local_root: String,
}