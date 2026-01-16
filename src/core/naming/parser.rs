use once_cell::sync::Lazy;
use regex::{Regex, RegexBuilder};
use std::collections::BTreeSet;

use super::types::SceneNameParts;

// Regexes for common tokens we want to salvage
static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(19|20)\d{2}").unwrap());
static RESOLUTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(480p|576p|720p|1080p|2160p|4k|8k)\b").unwrap());
static SOURCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(AMZN(\.WEB(-?DL)?)?|WEB(-?DL|Rip)?|Blu[- ]?Ray|BRRip|BDRip|WEBRip|WEB|HDLight|HDLigh|mHD)\b").unwrap()
});
static VCODEC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(x265|x264|h\.?265|h\.?264|hevc|avc)\b").unwrap());
static ACODEC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(DDP|EAC3|AC3|DTS(-HD)?|TrueHD|AAC)\b").unwrap());
static ACHANNELS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(7\.1|5\.1|6CH|2\.0|2CH)\b").unwrap());
static BITDEPTH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(10bit|8bit)\b").unwrap());
static HDR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(HDR10\+?|HDR|HLG)\b").unwrap());
static DV_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(DV|Dolby[ .]?Vision)\b").unwrap());
static MULTI_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(MULTI|MULTi|Multi|FRENCH|VFF|VFQ|VFI|VOA|VF2|FR2|TRUEFRENCH|FR|EN)\b")
        .unwrap()
});
static GROUP_RE: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r"-([A-Za-z0-9._⚡]+)$")
        .multi_line(false)
        .build()
        .unwrap()
});

pub fn parse_scene_name(input: &str) -> SceneNameParts {
    let mut parts = SceneNameParts::default();
    let s = input.trim();

    // Release group
    if let Some(caps) = GROUP_RE.captures(s) {
        if let Some(m) = caps.get(1) {
            parts.release_group = Some(m.as_str().to_string());
        }
    }

    // Year
    if let Some(m) = YEAR_RE.find(s) {
        let y = &s[m.start()..m.end()].parse::<u16>().ok();
        parts.year = *y;
    }

    // Resolution, source, codecs
    if let Some(m) = RESOLUTION_RE.find(s) {
        parts.resolution = Some(m.as_str().to_string());
    }
    if let Some(m) = SOURCE_RE.find(s) {
        parts.source = Some(m.as_str().to_string());
    }
    if let Some(m) = VCODEC_RE.find(s) {
        parts.video_codec = Some(m.as_str().to_string());
    }
    if let Some(m) = ACODEC_RE.find(s) {
        parts.audio_codec = Some(m.as_str().to_string());
    }
    if let Some(m) = ACHANNELS_RE.find(s) {
        parts.audio_channels = Some(m.as_str().to_string());
    }
    if let Some(m) = BITDEPTH_RE.find(s) {
        parts.bit_depth = Some(m.as_str().to_string());
    }
    if HDR_RE.is_match(s) {
        parts.hdr = true;
    }
    if DV_RE.is_match(s) {
        parts.dv = true;
    }

    // Languages/tags and extras: we salvage upper-case-ish tokens, including IMAX, 4KLight, etc.
    let mut extras: BTreeSet<String> = BTreeSet::new();
    for token in s
        .split(['.', ' ', '_', '(', ')', '[', ']', '-'])
        .filter(|t| !t.is_empty())
    {
        let t = token.trim();
        let lt = t.to_ascii_lowercase();
        // skip tokens that are clearly numeric years/resolutions/codecs already captured
        if YEAR_RE.is_match(t)
            || RESOLUTION_RE.is_match(t)
            || SOURCE_RE.is_match(t)
            || VCODEC_RE.is_match(t)
            || ACODEC_RE.is_match(t)
            || ACHANNELS_RE.is_match(t)
            || BITDEPTH_RE.is_match(t)
            || t == "WEB"
            || t == "Rip"
        {
            continue;
        }
        // common extras or language tokens
        let is_known = matches!(
            lt.as_str(),
            "imax" | "4klight" | "hdlight" | "vff" | "vfq" | "vfi" | "multi" | "french" | "atmos"
        ) || MULTI_RE.is_match(t);
        let is_all_upper =
            t.chars().all(|c| !c.is_ascii_lowercase()) && t.chars().any(|c| c.is_ascii_uppercase());
        if is_known || is_all_upper {
            extras.insert(t.to_string());
        }
    }
    parts.extra_tags = extras;

    // Title tokens: naive salvage – before the first year occurrence, normalized by dots
    let title_slice = if let Some(m) = YEAR_RE.find(s) {
        &s[..m.start()]
    } else {
        s
    };
    let title_tokens: Vec<String> = title_slice
        .split(['.', ' '])
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();
    if !title_tokens.is_empty() {
        parts.title_tokens = title_tokens;
    }

    parts
}
