use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::core::naming::TechnicalInfo;

fn parse_int_from_value(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() { None } else { digits.parse::<i64>().ok() }
        }
        _ => None,
    }
}

fn map_video_codec(format: &str) -> Option<String> {
    // Canonicalize to encoder-style tokens: x265/x264
    let f = format.to_ascii_lowercase();
    if f.contains("x265") || f.contains("hevc") || f.contains("h265") || f.contains("h.265") { Some("x265".to_string()) }
    else if f.contains("x264") || f.contains("avc") || f.contains("h264") || f.contains("h.264") { Some("x264".to_string()) }
    else { Some(format.to_string()) }
}

fn map_audio_codec(format: &str) -> Option<String> {
    let f = format.to_ascii_lowercase();
    if f.contains("e-ac-3") || f.contains("eac3") || f.contains("ddp") || f.contains("dolby digital plus") {
        Some("EAC3".to_string())
    } else if f.contains("ac-3") || f.contains("ac3") || f.contains("dolby digital") {
        Some("AC3".to_string())
    } else if f.contains("dts") { Some("DTS".to_string()) }
    else if f.contains("aac") { Some("AAC".to_string()) }
    else if f.contains("mpeg") { Some("MPEG".to_string()) }
    else { Some(format.to_string()) }
}

fn map_channels(ch: i64) -> Option<String> {
    match ch {
        8 => Some("7.1".to_string()),
        7 => Some("6.1".to_string()),
        6 => Some("5.1".to_string()),
        2 => Some("2.0".to_string()),
        _ => None,
    }
}

fn run_mediainfo_json(path: &str) -> Option<Vec<u8>> {
    let output = Command::new("mediainfo").arg("--Output=JSON").arg(path).output();
    let Ok(out) = output else { return None; };
    if !out.status.success() { return None; }
    Some(out.stdout)
}

fn run_mediainfo_text(path: &str) -> Option<String> {
    let output = Command::new("mediainfo").arg("--Output=Text").arg(path).output();
    let Ok(out) = output else { return None; };
    if !out.status.success() { return None; }
    String::from_utf8(out.stdout).ok()
}

fn get_modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn ensure_cache_and_load_json(video_path: &str, enable_cache: bool) -> Option<Value> {
    if !enable_cache {
        return run_mediainfo_json(video_path).and_then(|b| serde_json::from_slice(&b).ok());
    }

    let vpath = Path::new(video_path);
    let parent = vpath.parent().unwrap_or_else(|| Path::new("."));
    let json_path = parent.join("mediainfo.json");
    let nfo_path = parent.join("mediainfo.nfo");

    let v_mtime = get_modified_time(vpath);
    let j_mtime = get_modified_time(&json_path);
    let n_mtime = get_modified_time(&nfo_path);

    let need_refresh = match (v_mtime, j_mtime) {
        (Some(vm), Some(jm)) => jm < vm,
        (Some(_), None) => true,
        _ => false,
    };

    if need_refresh {
        debug!(target: "ghostseed::mediainfo", path = %video_path, "Refreshing mediainfo cache files");
        if let Some(text) = run_mediainfo_text(video_path) {
            let _ = fs::write(&nfo_path, text);
        }
        if let Some(bytes) = run_mediainfo_json(video_path) {
            let _ = fs::write(&json_path, &bytes);
            return serde_json::from_slice(&bytes).ok();
        }
        return None;
    }

    // Cache up-to-date: if json exists, read it; else if only nfo exists, generate json now
    if json_path.exists() {
        if let Ok(bytes) = fs::read(&json_path) { return serde_json::from_slice(&bytes).ok(); }
        return None;
    }
    // No json; if we have nfo and it's not older than video, produce json now
    if let (Some(vm), Some(nm)) = (v_mtime, n_mtime) {
        if nm >= vm {
            if let Some(bytes) = run_mediainfo_json(video_path) {
                let _ = fs::write(&json_path, &bytes);
                return serde_json::from_slice(&bytes).ok();
            }
        }
    }
    None
}

pub fn collect_technical_info_with_cache(path: &str, enable_cache: bool) -> TechnicalInfo {
    info!(target: "ghostseed::mediainfo", path = %path, cache = enable_cache, "Collecting technical info");
    let json_opt = ensure_cache_and_load_json(path, enable_cache);

    let mut info = TechnicalInfo::default();

    let Some(json) = json_opt else {
        warn!(target: "ghostseed::mediainfo", path = %path, "Failed to get mediainfo JSON (cache disabled or command failed)");
        return info;
    };
    let tracks = json
        .get("media").and_then(|m| m.get("track")).and_then(|t| t.as_array());
    let Some(tracks) = tracks else { return info; };

    for track in tracks {
        let ttype = track.get("@type").and_then(|v| v.as_str()).unwrap_or("");
        match ttype {
            "Video" => {
                // Resolution from Width preferred (convert width -> common p)
                if let Some(w) = track.get("Width") { if let Some(wn) = parse_int_from_value(w) {
                    let res = if wn >= 3800 { "2160p" } else if wn >= 2500 { "1440p" } else if wn >= 1900 { "1080p" } else if wn >= 1200 { "720p" } else { "480p" };
                    info.resolution = Some(res.to_string());
                }}
                // Fallback to Height if Width unavailable
                if info.resolution.is_none() {
                    if let Some(h) = track.get("Height") { if let Some(hn) = parse_int_from_value(h) {
                        let res = if hn >= 2160 { "2160p" } else if hn >= 1440 { "1440p" } else if hn >= 1080 { "1080p" } else if hn >= 720 { "720p" } else { "480p" };
                        info.resolution = Some(res.to_string());
                    }}
                }
                // Video codec
                if let Some(fmt) = track.get("Format").and_then(|v| v.as_str()) {
                    info.video_codec = map_video_codec(fmt);
                }
                // Bit depth: only report 10bit; ignore 8bit
                if let Some(bd) = track.get("BitDepth") {
                    if let Some(bits) = parse_int_from_value(bd) {
                        if bits >= 10 { info.bit_depth = Some("10bit".to_string()); }
                    }
                }
                // HDR/Dolby Vision
                if let Some(hdrf) = track.get("HDR_Format").and_then(|v| v.as_str()) {
                    let l = hdrf.to_ascii_lowercase();
                    if l.contains("hdr") { info.hdr = true; }
                    if l.contains("dolby vision") { info.dv = true; info.hdr = true; }
                }
                // Some files have Transfer_Characteristics or ColorPrimaries with PQ/HLG hints
                if let Some(tc) = track.get("transfer_characteristics").and_then(|v| v.as_str()) {
                    let l = tc.to_ascii_lowercase();
                    if l.contains("pq") || l.contains("hlg") { info.hdr = true; }
                }
            }
            "Audio" => {
                if let Some(fmt) = track.get("Format").and_then(|v| v.as_str()) {
                    info.audio_codec = map_audio_codec(fmt);
                }
                // Channels: prefer Channel(s), else Channel(s)_Original
                let chan_val = track.get("Channel(s)").or_else(|| track.get("Channel(s)_Original"));
                if let Some(cv) = chan_val { if let Some(n) = parse_int_from_value(cv) { info.audio_channels = map_channels(n); } }
                // Alternatively, if textual contains "5.1", accept it
                if info.audio_channels.is_none() {
                    let text = chan_val.and_then(|v| v.as_str());
                    if let Some(t) = text { if t.contains("5.1") { info.audio_channels = Some("5.1".to_string()); } }
                }
                // Audio language and VFI
                let lang = track.get("Language").and_then(|v| v.as_str())
                    .or_else(|| track.get("Language/String").and_then(|v| v.as_str()));
                if let Some(l) = lang { 
                    let lc = l.to_ascii_lowercase();
                    let code = if lc.starts_with("fr") { "fr" } else if lc.starts_with("en") { "en" } else { lc.as_str() };
                    info.audio_languages.insert(code.to_string());
                }
                let title = track.get("Title").and_then(|v| v.as_str()).unwrap_or("");
                if !title.is_empty() && title.to_ascii_uppercase().contains("VFI") { info.has_vfi = true; }
            }
            "Text" => {
                // Subtitle languages
                let lang = track.get("Language").and_then(|v| v.as_str())
                    .or_else(|| track.get("Language/String").and_then(|v| v.as_str()));
                if let Some(l) = lang { 
                    let lc = l.to_ascii_lowercase();
                    let code = if lc.starts_with("fr") { "fr" } else if lc.starts_with("en") { "en" } else { lc.as_str() };
                    info.subtitle_languages.insert(code.to_string());
                }
            }
            _ => {}
        }
    }

    info!(target: "ghostseed::mediainfo", path = %path, res = ?info.resolution, vcodec = ?info.video_codec, bitdepth = ?info.bit_depth, hdr = info.hdr, dv = info.dv, acodec = ?info.audio_codec, ach = ?info.audio_channels, alangs = ?info.audio_languages, slangs = ?info.subtitle_languages, vfi = info.has_vfi, "Collected technical info summary");
    info
}

/// Ensure a textual mediainfo is written to the provided output path.
/// Returns true on success.
pub fn write_text_nfo(video_path: &str, out_path: &Path) -> bool {
    if let Some(text) = run_mediainfo_text(video_path) {
        if let Some(parent) = out_path.parent() { let _ = fs::create_dir_all(parent); }
        return fs::write(out_path, text).is_ok();
    }
    false
}
