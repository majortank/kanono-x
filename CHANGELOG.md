# Changelog

## Unreleased

### Added

- Persistent SQLite-backed duplicate detection for Audius and configured external providers.
- Conservative matching by MusicBrainz recording ID, ISRC, optional `fpcalc` fingerprint, normalized metadata, duration, album, and track number.
- Duplicate download policies: skip/reuse, ask, allow duplicates, and explicit replace.
- Per-track and batch download reporting for downloaded, reused, and failed results.
- yt-dlp album, playlist, and track-number result metadata.
- Album-aware download organization under `Artist/Album`, with `Artist/Singles` for tracks without album metadata.

### Changed

- Provider downloads are staged before being moved into the organized library path.
- Library rescans update modified files and prune missing files under scanned roots.
