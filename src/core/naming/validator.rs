use once_cell::sync::Lazy;
use regex::Regex;

use super::types::{Issue, ValidationResult};

static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?P<year>(19|20)\d{2})").unwrap());

static RESOLUTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(480p|576p|720p|1080p|2160p|4k|8k)\b").unwrap()
});

static SOURCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(AMZN(\.WEB(-?DL)?)?|WEB(-?DL|Rip)?|Blu[- ]?Ray|BRRip|BDRip|HDLight|HDLigh|mHD)\b").unwrap()
});

static VIDEO_CODEC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(x265|x264|h\.?265|h\.?264|hevc|avc)\b").unwrap()
});

fn looks_dot_separated(name: &str) -> bool {
    let dot_count = name.matches('.').count();
    dot_count >= 2
}

pub fn validate_scene_name(name: &str) -> ValidationResult {
    let mut issues = Vec::new();

    let trimmed = name.trim();
    if trimmed.is_empty() {
        issues.push(Issue::Empty);
    } else {
        if trimmed.eq_ignore_ascii_case("unknown") {
            issues.push(Issue::IsUnknown);
        }
        if !looks_dot_separated(trimmed) {
            issues.push(Issue::MissingDots);
        }
        if !YEAR_RE.is_match(trimmed) {
            issues.push(Issue::MissingYear);
        }
        if !RESOLUTION_RE.is_match(trimmed) {
            issues.push(Issue::MissingResolution);
        }
        if !SOURCE_RE.is_match(trimmed) {
            issues.push(Issue::MissingSource);
        }
        if !VIDEO_CODEC_RE.is_match(trimmed) {
            issues.push(Issue::MissingVideoCodec);
        }
    }

    ValidationResult { valid: issues.is_empty(), issues }
}

pub fn is_scene_name_valid(name: &str) -> bool {
    validate_scene_name(name).valid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_examples() {
        let ok1 = "Rebel.Moon.Part.One.A.Child.of.Fire.2023.MULTi.1080p.WEB.x264-FW";
        let ok2 = "The.Blues.Brothers .1980-MULTI.(VFF-VO)-1080p-HDLigh.x264.ac3.mHDgz";
        assert!(is_scene_name_valid(ok1));
        assert!(is_scene_name_valid(ok2));
    }

    #[test]
    fn invalid_examples() {
        let bad1 = "Fight Club (1999) - VO-VF - 1080p - x265";
        let bad2 = "Bodies Bodies Bodies (2022) MULTi VFI 2160p 10bit 4KLight DV HDR BluRay DDP 5.1 Atmos x265-QTZ";
        let bad3 = "Everything Everywhere All at Once (2022)";
        let bad4 = "Unknown";

        assert!(!is_scene_name_valid(bad1));
        assert!(!is_scene_name_valid(bad2));
        assert!(!is_scene_name_valid(bad3));
        assert!(!is_scene_name_valid(bad4));

        let r = validate_scene_name(bad2);
        assert!(r.issues.iter().any(|i| matches!(i, Issue::MissingDots)));
        assert!(!r.issues.iter().any(|i| matches!(i, Issue::MissingVideoCodec)));
        assert!(!r.issues.iter().any(|i| matches!(i, Issue::MissingResolution)));
        assert!(!r.issues.iter().any(|i| matches!(i, Issue::MissingSource)));
    }
}
