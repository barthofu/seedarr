pub mod mediainfo;

use std::path::PathBuf;

use tracing::warn;

/// Translate a Radarr-reported container path to a local path using config mappings.
pub fn translate_radarr_path<'a>(radarr_path: &'a str, config: &'a crate::config::Config) -> PathBuf {
	let maps = &config.radarr.path_mappings;
	if maps.is_empty() {
		return PathBuf::from(radarr_path);
	}
	if let Some(local) = crate::utils::pathmap::translate_radarr_path(radarr_path, maps) {
		PathBuf::from(local)
	} else {
		warn!("No path mapping matched for radarr path: {}", radarr_path);
		PathBuf::from(radarr_path)
	}
}

// removed legacy single mapping helper; using radarr.path_mappings instead
