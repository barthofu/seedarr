use super::types::{DecisionReason, RadarrHints, SceneDecision, SceneNameParts, TechnicalInfo, ValidationResult};

fn normalize_tokens_to_scene<S: AsRef<str>>(s: S) -> String {
    // Replace whitespace and separators with dots, collapse, strip leading/trailing dots
    let mut out = String::with_capacity(s.as_ref().len());
    let mut last_dot = false;
    for ch in s.as_ref().chars() {
        let is_sep = ch.is_whitespace() || matches!(ch, ':' | ';' | ',' | '/' | '\\' | '|' | '!' | '?' | '\'' | '"' | '&');
        let is_dot = ch == '.' || is_sep;
        if is_dot {
            if !last_dot { out.push('.'); last_dot = true; }
        } else {
            last_dot = false;
            out.push(ch);
        }
    }
    out.trim_matches('.').to_string()
}

fn pick<T: Clone>(a: Option<T>, b: Option<T>) -> Option<T> {
    a.or(b)
}

fn canonicalize_video_codec(s: &str) -> String {
    let l = s.to_ascii_lowercase();
    if l.contains("265") { "x265".to_string() }
    else if l.contains("264") { "x264".to_string() }
    else { s.to_string() }
}

fn language_tag(tech: &TechnicalInfo) -> Option<String> {
    // MULTi.VF if multiple audio languages; VF if only French; VOSTFR if only English (assume FR subs)
    if tech.has_vfi { return Some("VFI".to_string()); }
    let count = tech.audio_languages.len();
    if count == 0 { return None; }
    if count > 1 { return Some("MULTi.VF".to_string()); }
    let only = tech.audio_languages.iter().next().unwrap().as_str();
    if only == "fr" { Some("VF".to_string()) }
    else if only == "en" { Some("VOSTFR".to_string()) }
    else { None }
}

fn merge_parts(mut parsed: SceneNameParts, hints: &RadarrHints, tech: &TechnicalInfo) -> SceneNameParts {
    // Title
    if parsed.title_tokens.is_empty() {
        let title = normalize_tokens_to_scene(&hints.title);
        if !title.is_empty() {
            parsed.title_tokens = title.split('.').map(|s| s.to_string()).collect();
        }
    }

    // Year
    if parsed.year.is_none() {
        parsed.year = hints.year;
    }

    // Resolution
    parsed.resolution = pick(tech.resolution.clone(), parsed.resolution);
    if parsed.resolution.is_none() {
        if let Some(q) = &hints.quality {
            let ql = q.to_ascii_lowercase();
            parsed.resolution = if ql.contains("2160") || ql.contains("4k") { Some("2160p".to_string()) }
                else if ql.contains("1440") { Some("1440p".to_string()) }
                else if ql.contains("1080") { Some("1080p".to_string()) }
                else if ql.contains("720") { Some("720p".to_string()) }
                else { None };
        }
    }
    if parsed.resolution.is_none() {
        if let Some(q) = &hints.quality {
            let ql = q.to_ascii_lowercase();
            if ql.contains("2160") || ql.contains("uhd") || ql.contains("4k") { parsed.resolution = Some("2160p".to_string()); }
            else if ql.contains("1080") || ql.contains("fhd") { parsed.resolution = Some("1080p".to_string()); }
            else if ql.contains("720") || ql.contains("hd") { parsed.resolution = Some("720p".to_string()); }
        }
    }
    // Source: retained from parsed; if missing try to infer from quality string a bit
    if parsed.source.is_none() {
        if let Some(q) = &hints.quality {
            let ql = q.to_ascii_lowercase();
            parsed.source = if ql.contains("web-dl") || ql.contains("webrip") || ql.contains("web") {
                Some("WEB".to_string())
            } else if ql.contains("blu") {
                Some("BluRay".to_string())
            } else { None };
        }
    }

    // Codecs, audio
    parsed.video_codec = pick(tech.video_codec.clone(), parsed.video_codec).map(|v| canonicalize_video_codec(&v));
    parsed.audio_codec = pick(tech.audio_codec.clone(), parsed.audio_codec);
    parsed.audio_channels = pick(tech.audio_channels.clone(), parsed.audio_channels);
    parsed.bit_depth = pick(tech.bit_depth.clone(), parsed.bit_depth);

    // HDR/DV flags
    parsed.hdr = tech.hdr || parsed.hdr;
    parsed.dv = tech.dv || parsed.dv;

    // Languages: parsed already contains a set; keep it

    // Release group: prefer parsed, else radarr
    if parsed.release_group.is_none() {
        parsed.release_group = hints.release_group.clone();
    }

    parsed
}

fn extras_to_vec(parts: &SceneNameParts) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut set: BTreeSet<String> = BTreeSet::new();

    if parts.hdr { set.insert("HDR".to_string()); }
    if parts.dv { set.insert("DV".to_string()); }
    if let Some(bd) = &parts.bit_depth { set.insert(bd.clone()); }

    for l in &parts.languages { set.insert(l.clone()); }

    // avoid duplicates: if HDR/DV/bitdepth already set, do not re-add from extra_tags
    for e in &parts.extra_tags {
        let el = e.to_ascii_uppercase();
        if (el == "HDR" && parts.hdr) || (el == "DV" && parts.dv) || (parts.bit_depth.as_ref().is_some() && el.contains("BIT")) {
            continue;
        }
        set.insert(e.clone());
    }

    set.into_iter().collect()
}

fn assemble(parts: &SceneNameParts) -> String {
    let mut segs: Vec<String> = Vec::new();

    if !parts.title_tokens.is_empty() {
        segs.push(parts.title_tokens.join("."));
    }
    if let Some(y) = parts.year { segs.push(y.to_string()); }

    // Language tag: place after title/year if present
    if let Some(tag) = &parts.language_tag { segs.push(tag.clone()); }

    // Common meta
    if let Some(res) = &parts.resolution { segs.push(res.clone()); }
    if let Some(src) = &parts.source { segs.push(src.clone()); }

    // Extras (HDR/DV/bitdepth/other tags)
    let extras = extras_to_vec(parts);
    for e in extras { segs.push(e); }

    if let Some(a) = &parts.audio_codec { segs.push(a.clone()); }
    if let Some(ch) = &parts.audio_channels { segs.push(ch.clone()); }

    // Video codec must be last
    if let Some(v) = &parts.video_codec { segs.push(v.clone()); }

    let mut name = segs.join(".");

    if let Some(group) = &parts.release_group {
        name.push('-');
        name.push_str(group);
    }

    name
}

/// Deterministically propose a scene name, optionally reusing info parsed from the original.
/// If `original` is present and valid, we accept it to avoid unnecessary churn.
pub fn propose_scene_name(original: Option<&str>, hints: &RadarrHints, tech: &TechnicalInfo, validation: Option<&ValidationResult>) -> SceneDecision {
    if let Some(orig) = original {
        if let Some(v) = validation { if v.valid { return SceneDecision { chosen: orig.to_string(), reason: DecisionReason::AcceptedExisting }; } }
    }

    // Parse original to salvage unknown-but-useful tags
    let mut parsed = original.map(super::parser::parse_scene_name).unwrap_or_default();
    // Compute language tag from technical info now so assemble can place it
    parsed.language_tag = language_tag(tech);
    let merged = merge_parts(parsed, hints, tech);
    let rebuilt = assemble(&merged);

    let reason = if let Some(v) = validation { super::types::DecisionReason::Rebuilt { issues: v.issues.clone() } } else { super::types::DecisionReason::Rebuilt { issues: vec![] } };

    SceneDecision { chosen: rebuilt, reason }
}
