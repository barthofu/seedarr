use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    Empty,
    IsUnknown,
    MissingDots,
    MissingYear,
    MissingResolution,
    MissingSource,
    MissingVideoCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneNameParts {
    pub title_tokens: Vec<String>,
    pub year: Option<u16>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<String>,
    pub bit_depth: Option<String>, // "10bit" etc.
    pub hdr: bool,
    pub dv: bool,
    pub languages: BTreeSet<String>,
    pub language_tag: Option<String>,
    pub release_group: Option<String>,
    pub extra_tags: BTreeSet<String>, // salvage: IMAX, 4KLight, VFF/VFQ, etc.
}

#[derive(Debug, Clone, Default)]
pub struct RadarrHints {
    pub title: String,
    pub year: Option<u16>,
    pub quality: Option<String>,
    pub release_group: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TechnicalInfo {
    pub resolution: Option<String>,
    pub video_codec: Option<String>,
    pub bit_depth: Option<String>,
    pub hdr: bool,
    pub dv: bool,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<String>,
    pub audio_languages: BTreeSet<String>,
    pub subtitle_languages: BTreeSet<String>,
    pub has_vfi: bool,
    pub container: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReason {
    AcceptedExisting,
    Rebuilt { issues: Vec<Issue> },
}

#[derive(Debug, Clone)]
pub struct SceneDecision {
    pub chosen: String,
    pub reason: DecisionReason,
}
