use std::path::Path;
use tracing::{debug, error, warn};

#[cfg(target_family = "unix")]
use std::os::unix::fs as unix_fs;

/// Export a seed structure for a given scene name and source video path:
/// <seed_root>/<scene_name>/<scene_name>.<ext>
/// <seed_root>/<scene_name>/<scene_name>.nfo
pub fn export_seed_structure(
    seed_root: &Path,
    scene_name: &str,
    src_video: &Path,
) -> std::io::Result<()> {
    let seed_dir = seed_root.join(scene_name);
    std::fs::create_dir_all(&seed_dir)?;

    // Symlink the video file as <scene>.<ext>
    let ext = src_video
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mkv");
    let dest_video = seed_dir.join(format!("{}.{}", scene_name, ext));
    // Idempotency: if video link already exists, skip entire export
    if dest_video.exists() {
        debug!("Seed export already exists: '{}'", dest_video.display());
        return Ok(());
    }

    #[cfg(target_family = "unix")]
    {
        // Prefer relative symlink target where possible
        let target =
            relative_target(&seed_dir, src_video).unwrap_or_else(|| src_video.to_path_buf());
        debug!(
            "Symlinking video: '{}' -> '{}'",
            target.display(),
            dest_video.display()
        );
        if let Err(e) = unix_fs::symlink(&target, &dest_video) {
            error!("Failed to symlink video '{}': {}", dest_video.display(), e);
        }
    }
    #[cfg(not(target_family = "unix"))]
    {
        // Fallback: hard-link then copy if hard-link fails
        if let Err(e) = std::fs::hard_link(src_video, &dest_video) {
            warn!(
                "Hard-link failed ({}), copying instead: '{}'",
                e,
                dest_video.display()
            );
            let _ = std::fs::copy(src_video, &dest_video);
        }
    }

    // NFO: prefer symlink of existing source mediainfo.nfo; else write textual mediainfo
    let dest_nfo = seed_dir.join(format!("{}.nfo", scene_name));
    // If NFO exists already, keep it
    if dest_nfo.exists() {
        return Ok(());
    }
    let src_nfo = src_video
        .parent()
        .unwrap_or(Path::new("."))
        .join("mediainfo.nfo");
    if src_nfo.exists() {
        #[cfg(target_family = "unix")]
        {
            let nfo_target =
                relative_target(&seed_dir, &src_nfo).unwrap_or_else(|| src_nfo.to_path_buf());
            if let Err(e) = unix_fs::symlink(&nfo_target, &dest_nfo) {
                warn!(
                    "Failed to symlink mediainfo.nfo ({}), generating new text NFO",
                    e
                );
                let _ = crate::core::media::mediainfo::write_text_nfo(
                    src_video.to_string_lossy().as_ref(),
                    &dest_nfo,
                );
            }
        }
        #[cfg(not(target_family = "unix"))]
        {
            if let Err(e) = std::fs::hard_link(&src_nfo, &dest_nfo) {
                warn!("Hard-link mediainfo.nfo failed ({}), copying instead", e);
                if let Err(e2) = std::fs::copy(&src_nfo, &dest_nfo) {
                    warn!("Copy mediainfo.nfo failed: {}", e2);
                }
            }
        }
    } else {
        let _ = crate::core::media::mediainfo::write_text_nfo(
            src_video.to_string_lossy().as_ref(),
            &dest_nfo,
        );
    }

    Ok(())
}

/// Compute a relative path from `from_dir` to `to_path` if they share a common ancestor.
#[cfg(target_family = "unix")]
fn relative_target(from_dir: &Path, to_path: &Path) -> Option<std::path::PathBuf> {
    if !from_dir.is_absolute() || !to_path.is_absolute() {
        return None;
    }
    // Find the deepest ancestor of `from_dir` that prefixes `to_path`
    let mut ancestor_opt = None;
    for anc in from_dir.ancestors() {
        if to_path.starts_with(anc) {
            ancestor_opt = Some(anc);
            break;
        }
    }
    let ancestor = ancestor_opt?;
    let from_suffix = from_dir.strip_prefix(ancestor).ok()?;
    let to_suffix = to_path.strip_prefix(ancestor).ok()?;
    let mut rel = std::path::PathBuf::new();
    let up_count = from_suffix.components().count();
    for _ in 0..up_count {
        rel.push("..");
    }
    rel.push(to_suffix);
    Some(rel)
}
