# Seedarr — Project Specifications

## Overview

**Seedarr** is a Rust-based automation tool designed to publish existing multimedia libraries
(Films and TV Series) to the BitTorrent network **without duplicating data**, primarily via **DHT**.

It leverages existing media managers (Radarr / Sonarr), technical media analysis, and configurable
naming rules to generate tracker-grade releases using **symlinks*- and **DHT-enabled torrents**.

The project is designed **DHT-first**, with a clear architectural path toward optional support for
private trackers in the future.

## Goals

- Publish media libraries to BitTorrent without file duplication
- Generate clean, standardized, collision-resistant release names
- Automate torrent creation and seeding
- Remain non-destructive to the source library
- Be extensible toward private trackers without design rewrites

## Non-Goals (Initial Scope)

- No private tracker upload APIs
- No ratio management
- No moderation / compliance logic
- No forced announce URLs
- No mandatory tracker-specific rules


## High-Level Architecture

```
Radarr / Sonarr
        ↓
Metadata Aggregation
        ↓
Media Analysis (MediaInfo)
        ↓
Naming Engine (DSL)
        ↓
Symlink Export Tree
        ↓
Torrent Generation (DHT)
        ↓
Seeding
```

## Metadata Sources

### Radarr / Sonarr (APIs)

Used as the primary metadata source:

- Title
- Year
- Season / Episode
- Quality (if available)
- Release group (best effort)
- Language / subtitles (when exposed)

### MediaInfo (Source of Truth)

Used to extract **actual technical properties**:

- Resolution
- Video codec / profile
- Bit depth
- HDR / Dolby Vision
- Audio codec / channels
- Container format

MediaInfo data always takes precedence over manager metadata.

## Naming Engine (Core Component)

The naming engine is the heart of Seedarr.

### Objectives

- Generate clean, standardized release names
- Minimize DHT collisions
- Remain configurable and future-proof
- Be tracker-grade without being tracker-bound

### DSL Requirements

- Declarative (non–Turing-complete)
- Template-based
- Conditional support (e.g. HDR, multi-audio)
- Normalization helpers (case, separators, cleanup)
- Profile-based (DHT / future trackers)

Example template:

```
{Title}.{Year}.{Resolution}.{Source}.{VideoCodec}.{Audio}.{Languages}-{Group}
```

## Symlink Strategy

- Seedarr never moves or copies media files
- A dedicated export directory is created
- Symlinks point to the original Radarr/Sonarr-managed files
- Enables:
  - Multiple naming strategies
  - Safe rollback
  - Zero storage overhead
  - Multi-publish workflows

## Torrent Generation (DHT-first)

### DHT Mode (Default)

- No announce URLs
- `private` flag disabled
- DHT, PEX, and LSD enabled
- Automatic piece size selection
- Optional source tag

### Seeding

- Torrents are seeded directly from symlinked paths
- Compatible with common clients:
  - qBittorrent
  - Transmission
  - rTorrent

## DHT-Specific Considerations

- Naming verbosity is encouraged (DHT ≠ curated index)
- Names must include:
  - Title
  - Year
  - Resolution
  - Codec
  - Language(s)
- Collision avoidance is a primary concern

## TV Series Handling

TV series introduce additional complexity:

- Single episode vs season packs
- Specials (S00E*)
- Multi-episode files
- Alternative numbering schemes

A dedicated TV-series naming and packaging logic is required.

## CLI & UX Principles

- CLI-first
- Explicit workflow stages:
  ```
  scan → validate → link → torrent → seed
  ```
- Dry-run mode is mandatory
- Verbose logging
- Deterministic outputs
- CI-friendly exit codes

## Extensibility: Private Trackers (Future)

Seedarr is architected to support private trackers later without refactoring.

### Publisher Abstraction

```
Publisher
├── DHTPublisher
└── TrackerPublisher (future)
```

### Naming Profiles

- `profiles/dht.yaml`
- `profiles/<tracker>.yaml`

### Capability Flags

```yaml
features:
  dht: true
  private: false
  announce: false
```

Future tracker profiles:

```yaml
features:
  dht: false
  private: true
  announce: true
```

## Suggested Roadmap

### Phase 1 — Proof of Concept

- Films only
- MediaInfo integration
- Symlink export
- DHT torrent generation
- Basic CLI

### Phase 2 — Quality & Robustness

- TV series support
- Naming DSL implementation
- Validation & dry-run
- Collision handling

### Phase 3 — Distribution

- Automatic seeding
- Torrent rebuilds without reseeding
- Monitoring & status

### Phase 4 — Private Trackers

- Private flag support
- Announce URLs
- Upload APIs
- Tracker-specific naming profiles

## Summary

Seedarr aims to be a **clean, reproducible, and extensible BitTorrent publishing tool**.

It focuses on:
- correctness over convenience
- automation without data duplication
- strong naming guarantees
- long-term maintainability

DHT-first is not a limitation but a design strength.
