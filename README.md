# Seedarr

Seedarr is a Rust-based automation tool to publish existing media libraries (films first) to BitTorrent without duplicating data. It builds deterministic, tracker-grade scene names, exports a symlinked seed tree, and generates torrents (DHT-first, with optional tracker announce) using the Intermodal CLI.

## Features

- Deterministic scene naming built from Radarr + MediaInfo only
- MediaInfo caching next to source files (`mediainfo.json` + `mediainfo.nfo`)
- Configurable path mappings to translate Radarr Docker paths to local filesystem
- Symlink export tree under a configurable `seed_path`
- Relative symlink targets when possible (portable across mounts)
- Torrent generation via Intermodal (`imdl`), configurable announce + private
- Idempotent operations: skip if symlinks or `.torrent` already exist
- Dry-run mode: create symlinks only, skip torrent creation

## Requirements

- Linux
- Rust toolchain (cargo)
- MediaInfo CLI (`mediainfo` on PATH)
- Intermodal (`imdl` on PATH): https://github.com/casey/intermodal

## Quick Start

1) Configure Seedarr via `config.toml` (or set `SEEDARR_CONFIG_PATH` to your file):

```toml
[logs]
level = "debug"
enable_reqwest_logging = false

[media]
use_original_title = true
enable_mediainfo_cache = true
seed_path = "/data/medias/seed" # symlink export root

[torrent]
announce_url = "https://tracker.example/announce/XYZ" # optional
private = true
output_dir = "/data/medias/torrents" # optional, default: seed scene dir
dry_run = false # when true, skip torrent creation

[[radarr.path_mappings]]
radarr_root = "/data/library/movies"
local_root = "/data/medias/plex/library/movies"

[[radarr.path_mappings]]
radarr_root = "/data/library/animation"
local_root = "/data/medias/plex/library/animation"
```

2) Run Seedarr:

```sh
cargo run
```

Seedarr will:
- Fetch Radarr movies via API
- Translate Radarr container paths to local paths using `radarr.path_mappings`
- Collect MediaInfo (JSON, cached) and build the canonical scene name
- Export symlinks into `seed_path/<scene>/<scene>.<ext>` and `<scene>.nfo`
- Create `<scene>.torrent` with Intermodal (unless `dry_run`)6

## Scene Naming Rules (current)

- Always rebuilt from Radarr hints + MediaInfo (original names ignored)
- Title sanitization: spaces, hyphens, brackets → dots; collapse multiple separators
- Language tag:
	- `MULTi.VF` when multiple audio languages
	- `VF` when only French, `VOSTFR` when only English
	- `VFI` added as an extra tag when detected
- Resolution: prefer MediaInfo width-derived; if it mismatches quality-implied resolution, trust MediaInfo and omit `source`
- Source: inferred (e.g., `WEB`, `BluRay`) when consistent with quality
- Video codec: canonicalized to `x265|x264` and placed last
- Bit depth: only show `10bit` (hide `8bit`)
- HDR/DV: include `HDR`/`DV` extras when detected (DV implies HDR)
- Special tags salvaged case-insensitively from Radarr-provided names: `IMAX`, `HDLight`, `4KLight`, `Unrated`, `Extended`, `Remastered`, `Directors.Cut`, `Theatrical.Cut`, `Proper`, `Repack`
- Release group: appended as `-Group` suffix when available

Example assembled name:

```
Interstellar.2014.MULTi.VF.2160p.BluRay.10bit.HDR.VFI.AC3.x265-QTZ
```

## MediaInfo Cache

- When `enable_mediainfo_cache = true`, Seedarr writes `mediainfo.json` and `mediainfo.nfo` next to the source video path, refreshing them when the video file is newer.
- The seed folder includes `<scene>.nfo`, symlinked to the source `mediainfo.nfo` (or generated on the fly).

## Symlink Export

- Layout: `seed_path/<scene_name>/<scene_name>.<ext>` and `seed_path/<scene_name>/<scene_name>.nfo`
- Relative symlinks are used when the seed directory and source share a common ancestor; otherwise absolute paths are used.
- Idempotent: if `<scene_name>.<ext>` exists, export is skipped.

## Torrent Creation (Intermodal)

- Command executed (conceptually):

```sh
imdl torrent create --follow-symlinks [--private] [-a <announce>] --output <out>/<scene>.torrent <seed_path>/<scene>
```

- Config flags:
	- `torrent.private`: adds `--private`
	- `torrent.announce_url`: adds `-a <url>`
	- `torrent.output_dir`: writes `.torrent` files there; default is the seed scene directory
	- `torrent.dry_run`: when true, skip torrent creation entirely
- Idempotent: if the target `.torrent` file already exists, it’s skipped

## Safety & Skips

- `seed_path` is verified at startup; created if missing.
- Movies whose paths do not match any `radarr.path_mappings` entry are skipped.

## Roadmap (high level)

- Films: complete flow (current focus)
- TV series: dedicated naming and packaging
- Naming DSL profiles (DHT / future trackers)
- Torrent generation enhancements (piece size, multi-file packs, metadata)
- Optional private tracker support (announce URLs, upload APIs)

## Environment & Configuration

- Config is loaded from `./config.toml` by default; override with `SEEDARR_CONFIG_PATH` env.
- Logging controlled via `[logs]` section.

## Notes

- Seedarr never copies or moves source video files; it relies on symlinks and shared storage.
- Designed DHT-first; verbose naming minimizes collisions and improves discoverability.
