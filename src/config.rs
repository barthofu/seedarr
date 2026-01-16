use serde::Deserialize;

impl Config {
    pub fn init() -> Result<Self, config::ConfigError> {
        // get config toml dir from env, with default
        let config_path =
            std::env::var("SEEDARR_CONFIG_PATH").unwrap_or_else(|_| String::from("./config.toml"));

        let config = config::Config::builder()
            // Add in config toml
            .add_source(config::File::with_name(&config_path))
            // Add in settings from the environment (with a prefix of SEEDARR)
            .add_source(config::Environment::with_prefix("SEEDARR").separator("__"))
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
    pub media: MediaConfig,
    pub torrent: TorrentConfig,
    pub radarr: RadarrConfig,
    #[serde(default)]
    pub upload: Option<UploadConfig>,
    pub paths: Option<PathsConfig>,
}

// ===============================================================================
// Upload (private trackers)
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct UploadConfig {
    /// Dry run: skip upload calls (keeps torrent creation behavior unchanged)
    #[serde(default)]
    pub dry_run: bool,

    /// Deprecated single-tracker selector (kept for backward compatibility).
    /// Prefer enabling each tracker block via `[upload.<tracker>].enable = true`.
    #[serde(default)]
    pub tracker: Option<String>,

    /// Tracker-specific configuration blocks
    #[serde(default)]
    pub torrust: Option<TorrustUploadConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct TorrustUploadConfig {
    #[serde(default)]
    pub enable: bool,
    pub api_base: String,
    pub api_key: String,
    #[serde(default)]
    pub movies_category: Option<String>,
    #[serde(default)]
    pub series_category: Option<String>,
    #[serde(default)]
    pub animes_category: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

// ===============================================================================
// Media
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct MediaConfig {
    pub use_original_title: bool,
    #[serde(default)]
    pub enable_mediainfo_cache: bool,
    #[serde(default)]
    pub seed_path: Option<String>,
    /// When true, append "-NoTag" to scene name if release group is missing
    #[serde(default)]
    pub append_no_tag_on_missing_group: bool,
    /// Title selection strategy (if set, overrides `use_original_title`)
    #[serde(default)]
    pub title_strategy: Option<TitleStrategy>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleStrategy {
    /// Use original_title if original_language is English; otherwise use localized title
    OriginalIfEnElseLocal,
    /// Always use the localized title
    AlwaysLocal,
}

// ===============================================================================
// Torrent
// ===============================================================================

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct TorrentConfig {
    /// Announce URL for trackers (optional). If None, no announce is added.
    pub announce_url: Option<String>,
    /// Whether to mark the torrent as private.
    #[serde(default = "default_true")]
    pub private: bool,
    /// Optional directory to write .torrent files. If None, use the seed scene dir.
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Dry run: only create symlinks, skip torrent creation.
    #[serde(default)]
    pub dry_run: bool,
}

fn default_true() -> bool {
    true
}

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
