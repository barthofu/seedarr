use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn, error};

/// Create a .torrent for the given seed directory using the intermodal CLI.
/// Output file: <seed_dir>/<scene_name>.torrent
pub fn create_torrent_for_seed_dir(seed_dir: &Path, scene_name: &str, config: &crate::config::Config) -> std::io::Result<PathBuf> {
    let output_root = config.torrent.output_dir.as_ref().map(|s| PathBuf::from(s)).unwrap_or_else(|| seed_dir.to_path_buf());
    let output = output_root.join(format!("{}.torrent", scene_name));

    // Idempotency: skip if torrent already exists
    if output.exists() {
        info!("Torrent already exists: '{}' — skipping", output.display());
        return Ok(output);
    }

    let mut cmd = Command::new("imdl");
    cmd.arg("torrent").arg("create")
        .arg("--follow-symlinks");

    if config.torrent.private { cmd.arg("--private"); }
    if let Some(url) = &config.torrent.announce_url { cmd.arg("-a").arg(url); }

    // Set explicit output to avoid surprises
    cmd.arg("--output").arg(&output);
    // Source directory (the seed dir with symlinks)
    cmd.arg(seed_dir);

    info!("Creating torrent via intermodal: '{}'", output.display());
    match cmd.output() {
        Ok(out) => {
            if out.status.success() {
                info!("Torrent created: '{}'", output.display());
                Ok(output)
            } else {
                error!("intermodal exited with status {:?}. stderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
                Err(std::io::Error::new(std::io::ErrorKind::Other, "intermodal failed"))
            }
        }
        Err(e) => {
            warn!("Failed to spawn 'imdl': {}. Is intermodal installed?", e);
            Err(e)
        }
    }
}
