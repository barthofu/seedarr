# Copilot Instructions — Seedarr

These instructions provide Copilot with the core functional specs, technical architecture, and project layout to assist effectively on this codebase.

## Functional Overview

- Deterministic scene-name builder using Radarr metadata + MediaInfo. Always rebuild names; do not trust existing names.
- Path mapping: translate Radarr container paths to local filesystem via `radarr.path_mappings`. Strict: skip items without a matching mapping.
- MediaInfo caching: write `mediainfo.json` and `mediainfo.nfo` next to the source video. Refresh when the source video mtime is newer. Toggle via `media.enable_mediainfo_cache`.
- Symlink export: for each scene name, create a seed directory containing:
  - `<seed_path>/<scene>/<scene>.<ext>` — symlink to the original file (prefer relative symlinks when possible)
  - `<seed_path>/<scene>/<scene>.nfo` — symlink or generated mediainfo text
  Idempotent: skip if already present.
- Torrent creation via Intermodal (`imdl torrent create`):
  - Follows symlinks, can set `--private`, optional `-a <announce>` URL.
  - Output `.torrent` path configurable; idempotent (skip if exists).
  - `torrent.dry_run` skips torrent creation (symlinks only).

## Naming Rules

- Title selection (configurable):
  - `original_if_en_else_local`: use `original_title` when original language is English; otherwise use localized `title`.
  - `always_local`: always use localized `title`.
  - Legacy `use_original_title` is honored only if `title_strategy` is not set.
- Sanitization:
  - Normalize separators: spaces, hyphens, brackets, etc. become dots; collapse multiple dots.
  - Remove atypical special symbols globally (e.g., ©, ®, ™, ℗) but keep normal letters (including accents), digits, dots, and hyphens.
  - Release group sanitized separately: strip spaces and non-alphanumerics. If empty or missing and `append_no_tag_on_missing_group = true`, append `-NoTag`.
- Language tags:
  - `MULTi.VF` if multiple audio languages
  - `VF` if only French; `VOSTFR` if only English
  - `VFI` added as extra tag when detected from audio track title
- Resolution: classify from MediaInfo using tolerant thresholds (handles crops like width=1915 as 1080p). If Radarr quality implies a different res, trust MediaInfo and drop `source`.
- Technical tokens:
  - Video codec canonicalized to `x265` / `x264` and placed last.
  - Do not include generic legacy video formats: drop `MPEG-4 Visual` and `MPEG Video`.
  - Bit depth: include only `10bit` (hide 8bit).
  - HDR/DV: include `HDR` and/or `DV` extras; DV implies HDR.
  - Audio: map common codecs (EAC3/AC3/DTS/AAC/MPEG) and channels (7.1/5.1/2.0).
- Extras salvage from original names (case-insensitive): IMAX, HDLight, 4KLight, Unrated, Extended, Remastered, Directors.Cut, Theatrical.Cut, Proper, Repack.
- Release group: appended as `-Group` suffix; sanitized as noted above.

## Technical Architecture

- `src/main.rs`: Orchestrates config, Radarr fetch, path mapping, MediaInfo collection (with cache), name proposal, seed export, and torrent creation.
- `src/config.rs`: Configuration models and loader. Sections: `logs`, `media`, `torrent`, `radarr` (with `path_mappings`).
- `src/core/`:
  - `naming/`
    - `types.rs`: data structures for naming parts and decisions.
    - `validator.rs`: basic scene-name validation.
    - `parser.rs`: parsing support (regexes for common tokens; used for salvage/validation).
    - `builder.rs`: deterministic builder; language tags; extras; codec canonicalization; sanitization; release group handling.
  - `media/`
    - `mediainfo.rs`: run MediaInfo (JSON/Text), caching, extract technical info (resolution, codecs, languages, HDR/DV, bit depth).
    - `mod.rs`: path translation helpers (`try_translate_radarr_path`).
  - `fs/mod.rs`: seed export: create scene dir, relative symlink to video, `.nfo` handling, idempotency.
  - `torrent/mod.rs`: wrapper for `imdl torrent create` with `--follow-symlinks`, `--private`, `-a`, and output path.
- `src/utils/`: utilities and error types.

## Project Layout

- `Cargo.toml`
- `config.toml` (local config) and `config.toml.example` (template)
- `README.md`
- `docs/specs.md`
- `src/` (see architecture above)

## Configuration Reference (TOML)

```
[logs]
level = "info"            # error|warn|info|debug|trace
enable_reqwest_logging = false

[media]
use_original_title = false # deprecated; ignored when title_strategy is set
enable_mediainfo_cache = true
seed_path = "/mnt/media/seed"
append_no_tag_on_missing_group = true
# original_if_en_else_local | always_local
title_strategy = "original_if_en_else_local"

[torrent]
announce_url = "https://tracker.example/announce/ABC123" # optional
private = true
output_dir = "/mnt/media/torrents"                        # optional
dry_run = false

[radarr]
base_url = "http://localhost:7878"
api_key  = "YOUR_RADARR_API_KEY"

[[radarr.path_mappings]]
radarr_root = "/data/library/movies"
local_root  = "/mnt/media/library/movies"
```

- Env override: set `GHOSTSEED_CONFIG_PATH` to use a non-default config path.

## External Dependencies

- MediaInfo CLI (`mediainfo` on PATH)
- Intermodal CLI (`imdl` on PATH)
- Radarr API reachable with configured `base_url` and `api_key`

## Conventions & Notes

- Name assembly order: Title.Year.[LanguageTag].[Resolution].[Source].[Extras].[AudioCodec].[AudioChannels].[VideoCodec]-Group
- Video codec must be last. Source is omitted when resolution inferred from Radarr quality mismatches MediaInfo.
- Symlinks attempt to be relative when possible for portability.
- Operations are idempotent: re-runs do not duplicate symlinks or torrents.

## Common Tasks

- Build: `cargo check`
- Run: `cargo run`
- Adjust naming rules: edit `src/core/naming/builder.rs` (language tag logic, extras, sanitization, codec placement).
- Tune resolution thresholds: `src/core/media/mediainfo.rs` (`classify_resolution`).
- Update torrent behavior: `src/core/torrent/mod.rs`.

## Roadmap (High Level)

- TV series support (Sonarr)
- Naming profiles per tracker/DHT
- Torrent generation options (piece size, metadata)
- Optional tracker integrations
