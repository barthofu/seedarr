use std::path::PathBuf;
use std::str::FromStr;

use tracing::Level;

pub fn init_logging(config: &crate::config::Config) {
    tracing_subscriber::fmt()
        .with_max_level(Level::from_str(&config.logs.level).unwrap_or(Level::INFO))
        .init();
}

pub fn ensure_seed_path(config: &crate::config::Config) -> Result<(), String> {
    let Some(seed_root) = &config.media.seed_path else {
        return Ok(());
    };

    let seed_path = PathBuf::from(seed_root);
    if seed_path.exists() {
        if !seed_path.is_dir() {
            return Err(format!(
                "Configured seed_path '{}' exists but is not a directory",
                seed_path.display()
            ));
        }
        return Ok(());
    }

    std::fs::create_dir_all(&seed_path).map_err(|e| {
        format!(
            "Failed to create configured seed_path '{}': {}",
            seed_path.display(),
            e
        )
    })?;
    tracing::info!("Created seed_path directory: '{}'", seed_path.display());
    Ok(())
}

pub fn apply_resolution_fallback(
    tech: &mut crate::core::naming::TechnicalInfo,
    quality: Option<&str>,
) {
    if tech.resolution.is_some() {
        return;
    }
    let Some(q) = quality else {
        return;
    };

    let ql = q.to_ascii_lowercase();
    tech.resolution = if ql.contains("2160p") || ql.contains("4k") {
        Some("2160p".to_string())
    } else if ql.contains("1440p") {
        Some("1440p".to_string())
    } else if ql.contains("1080p") {
        Some("1080p".to_string())
    } else if ql.contains("720p") {
        Some("720p".to_string())
    } else {
        None
    };
}
