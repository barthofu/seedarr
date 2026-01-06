use std::path::Path;
use tracing::{debug, error, warn};

#[cfg(target_family = "unix")]
use std::os::unix::fs as unix_fs;

/// Export a seed structure for a given scene name and source video path:
/// <seed_root>/<scene_name>/<scene_name>.<ext>
/// <seed_root>/<scene_name>/<scene_name>.nfo
pub fn export_seed_structure(seed_root: &Path, scene_name: &str, src_video: &Path) -> std::io::Result<()> {
    let seed_dir = seed_root.join(scene_name);
    std::fs::create_dir_all(&seed_dir)?;

    // Symlink the video file as <scene>.<ext>
    let ext = src_video.extension().and_then(|s| s.to_str()).unwrap_or("mkv");
    let dest_video = seed_dir.join(format!("{}.{}", scene_name, ext));
    if dest_video.exists() { let _ = std::fs::remove_file(&dest_video); }

    #[cfg(target_family = "unix")]
    {
        debug!("Symlinking video: '{}' -> '{}'", src_video.display(), dest_video.display());
        if let Err(e) = unix_fs::symlink(src_video, &dest_video) {
            error!("Failed to symlink video '{}': {}", dest_video.display(), e);
        }
    }
    #[cfg(not(target_family = "unix"))]
    {
        // Fallback: hard-link then copy if hard-link fails
        if let Err(e) = std::fs::hard_link(src_video, &dest_video) {
            warn!("Hard-link failed ({}), copying instead: '{}'", e, dest_video.display());
            let _ = std::fs::copy(src_video, &dest_video);
        }
    }

    // NFO: prefer symlink of existing source mediainfo.nfo; else write textual mediainfo
    let dest_nfo = seed_dir.join(format!("{}.nfo", scene_name));
    if dest_nfo.exists() { let _ = std::fs::remove_file(&dest_nfo); }
    let src_nfo = src_video.parent().unwrap_or(Path::new(".")).join("mediainfo.nfo");
    if src_nfo.exists() {
        #[cfg(target_family = "unix")]
        {
            if let Err(e) = unix_fs::symlink(&src_nfo, &dest_nfo) {
                warn!("Failed to symlink mediainfo.nfo ({}), generating new text NFO", e);
                let _ = crate::core::media::mediainfo::write_text_nfo(src_video.to_string_lossy().as_ref(), &dest_nfo);
            }
        }
        #[cfg(not(target_family = "unix"))]
        {
            if let Err(e) = std::fs::hard_link(&src_nfo, &dest_nfo) {
                warn!("Hard-link mediainfo.nfo failed ({}), copying instead", e);
                if let Err(e2) = std::fs::copy(&src_nfo, &dest_nfo) { warn!("Copy mediainfo.nfo failed: {}", e2); }
            }
        }
    } else {
        let _ = crate::core::media::mediainfo::write_text_nfo(src_video.to_string_lossy().as_ref(), &dest_nfo);
    }

    Ok(())
}
