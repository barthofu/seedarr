use crate::config::PathMap;
use crate::config::SonarrPathMap;

/// Translate a Radarr path to local absolute path using longest-prefix mapping.
pub fn translate_radarr_path<'a>(radarr_path: &'a str, mappings: &'a [PathMap]) -> Option<String> {
    // choose longest matching radarr_root prefix
    let mut best: Option<&PathMap> = None;
    for m in mappings {
        let rr = m.radarr_root.trim_end_matches('/');
        if radarr_path.starts_with(rr) {
            match best {
                None => best = Some(m),
                Some(b) => {
                    if rr.len() > b.radarr_root.trim_end_matches('/').len() {
                        best = Some(m);
                    }
                }
            }
        }
    }
    if let Some(map) = best {
        let rr = map.radarr_root.trim_end_matches('/');
        let suffix = &radarr_path[rr.len()..];
        let local = format!("{}{}", map.local_root.trim_end_matches('/'), suffix);
        Some(local)
    } else {
        None
    }
}

/// Translate a Sonarr path to local absolute path using longest-prefix mapping.
pub fn translate_sonarr_path<'a>(
    sonarr_path: &'a str,
    mappings: &'a [SonarrPathMap],
) -> Option<String> {
    let mut best: Option<&SonarrPathMap> = None;
    for m in mappings {
        let rr = m.sonarr_root.trim_end_matches('/');
        if sonarr_path.starts_with(rr) {
            match best {
                None => best = Some(m),
                Some(b) => {
                    if rr.len() > b.sonarr_root.trim_end_matches('/').len() {
                        best = Some(m);
                    }
                }
            }
        }
    }
    if let Some(map) = best {
        let rr = map.sonarr_root.trim_end_matches('/');
        let suffix = &sonarr_path[rr.len()..];
        let local = format!("{}{}", map.local_root.trim_end_matches('/'), suffix);
        Some(local)
    } else {
        None
    }
}
