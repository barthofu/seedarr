use super::types::{DecisionReason, RadarrHints, SceneDecision, SceneNameParts, TechnicalInfo, ValidationResult};
use std::collections::BTreeSet;

fn normalize_tokens_to_scene<S: AsRef<str>>(s: S) -> String {
    // Replace whitespace and separators with dots, collapse, strip leading/trailing dots
    let mut out = String::with_capacity(s.as_ref().len());
    let mut last_dot = false;
    for ch in s.as_ref().chars() {
        let is_sep = ch.is_whitespace() || matches!(ch, ':' | ';' | ',' | '/' | '\\' | '|' | '!' | '?' | '\'' | '"' | '&' | '-' | '(' | ')' | '[' | ']' | '{' | '}');
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

// removed: pick helper no longer needed

fn canonicalize_video_codec(s: &str) -> String {
    let l = s.to_ascii_lowercase();
    if l.contains("265") { "x265".to_string() }
    else if l.contains("264") { "x264".to_string() }
    else { s.to_string() }
}

fn language_tag(tech: &TechnicalInfo) -> Option<String> {
    // MULTi.VF if multiple audio languages; VF if only French; VOSTFR if only English (assume FR subs)
    let count = tech.audio_languages.len();
    if count == 0 { return None; }
    if count > 1 { return Some("MULTi.VF".to_string()); }
    let only = tech.audio_languages.iter().next().unwrap().as_str();
    if only == "fr" { Some("VF".to_string()) }
    else if only == "en" { Some("VOSTFR".to_string()) }
    else { None }
}

fn infer_resolution_from_quality(q: &Option<String>) -> Option<String> {
    if let Some(qs) = q {
        let ql = qs.to_ascii_lowercase();
        if ql.contains("2160") || ql.contains("uhd") || ql.contains("4k") { return Some("2160p".to_string()); }
        if ql.contains("1440") { return Some("1440p".to_string()); }
        if ql.contains("1080") || ql.contains("fhd") { return Some("1080p".to_string()); }
        if ql.contains("720") || ql.contains("hd") { return Some("720p".to_string()); }
    }
    None
}

fn infer_source_from_quality(q: &Option<String>) -> Option<String> {
    if let Some(qs) = q {
        let ql = qs.to_ascii_lowercase();
        if ql.contains("web-dl") || ql.contains("webrip") || ql.contains("web") { return Some("WEB".to_string()); }
        if ql.contains("blu") { return Some("BluRay".to_string()); }
    }
    None
}

fn build_parts_from(hints: &RadarrHints, tech: &TechnicalInfo) -> SceneNameParts {
    let mut parts = SceneNameParts::default();

    // Title + year
    let title = normalize_tokens_to_scene(&hints.title);
    if !title.is_empty() { parts.title_tokens = title.split('.').map(|s| s.to_string()).collect(); }
    parts.year = hints.year;

    // Language tag
    parts.language_tag = language_tag(tech);

    // Resolution + source: prefer MediaInfo resolution; drop source if mismatch with quality-inferred resolution
    let inferred_res = infer_resolution_from_quality(&hints.quality);
    let inferred_source = infer_source_from_quality(&hints.quality);

    parts.resolution = tech.resolution.clone().or(inferred_res.clone());
    parts.source = inferred_source;

    if let (Some(tr), Some(ir)) = (tech.resolution.as_ref(), inferred_res.as_ref()) {
        if tr != ir {
            parts.resolution = Some(tr.clone());
            parts.source = None; // mismatch: trust MediaInfo, omit source
        }
    }

    // Technical extras
    parts.hdr = tech.hdr;
    parts.dv = tech.dv;
    parts.bit_depth = tech.bit_depth.clone();
    parts.audio_codec = tech.audio_codec.clone();
    parts.audio_channels = tech.audio_channels.clone();
    parts.video_codec = tech.video_codec.as_ref().map(|v| canonicalize_video_codec(v));

    // VFI: keep MULTi if present; add VFI as extra tag
    if tech.has_vfi { parts.extra_tags.insert("VFI".to_string()); }

    // Release group from Radarr
    parts.release_group = hints.release_group.clone();

    parts
}

fn salvage_special_tags(original: Option<&str>) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let Some(s) = original else { return tags; };
    let l = s.to_ascii_lowercase();

    // Requested tags
    if l.contains("imax") { tags.insert("IMAX".to_string()); }
    if l.contains("hdlight") { tags.insert("HDLight".to_string()); }
    if l.contains("4klight") || l.contains("4k light") { tags.insert("4KLight".to_string()); }

    // Common scene extras
    if l.contains("unrated") { tags.insert("Unrated".to_string()); }
    if l.contains("extended") { tags.insert("Extended".to_string()); }
    if l.contains("remastered") { tags.insert("Remastered".to_string()); }
    if l.contains("director") && l.contains("cut") { tags.insert("Directors.Cut".to_string()); }
    if l.contains("theatrical cut") { tags.insert("Theatrical.Cut".to_string()); }
    if l.contains("proper") { tags.insert("Proper".to_string()); }
    if l.contains("repack") { tags.insert("Repack".to_string()); }

    tags
}

// removed: merge_parts; we now build from hints + tech deterministically

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
    // Always rebuild deterministically from Radarr hints + MediaInfo
    let mut parts = build_parts_from(hints, tech);
    // Salvage special tags from original scene name (case-insensitive)
    for t in salvage_special_tags(original) { parts.extra_tags.insert(t); }
    let rebuilt = assemble(&parts);

    let reason = if let Some(v) = validation { DecisionReason::Rebuilt { issues: v.issues.clone() } } else { DecisionReason::Rebuilt { issues: vec![] } };

    SceneDecision { chosen: rebuilt, reason }
}
