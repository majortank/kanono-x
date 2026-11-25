use rusqlite::{params, Connection, OptionalExtension, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

use crate::library::Track;

pub struct LibraryDb {
    conn: Connection,
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct DbStats {
    pub total_tracks: usize,
    pub total_artists: usize,
    pub total_albums: usize,
    pub total_duration_secs: f32,
    pub total_plays: u64,
    pub db_file_size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicatePolicy {
    #[default]
    Skip,
    Ask,
    Allow,
    Replace,
}

impl DuplicatePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Ask => "ask",
            Self::Allow => "allow",
            Self::Replace => "replace",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "ask" => Self::Ask,
            "allow" => Self::Allow,
            "replace" => Self::Replace,
            _ => Self::Skip,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Skip => "Skip existing tracks",
            Self::Ask => "Ask before replacing",
            Self::Allow => "Allow duplicates",
            Self::Replace => "Replace existing (explicit policy)",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct DuplicateCandidate {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: f32,
    pub track_number: Option<u32>,
    pub musicbrainz_recording_id: Option<String>,
    pub isrc: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchReason {
    MusicBrainzRecordingId,
    Isrc,
    Fingerprint,
    Metadata,
}

impl MatchReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::MusicBrainzRecordingId => "MusicBrainz recording ID",
            Self::Isrc => "ISRC",
            Self::Fingerprint => "audio fingerprint",
            Self::Metadata => "normalized artist/title and duration",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DuplicateMatch {
    pub track: Track,
    pub reason: MatchReason,
}

#[derive(Clone, Debug)]
pub enum DownloadDisposition {
    Downloaded,
    Reused,
}

#[derive(Clone, Debug)]
pub struct DownloadResult {
    pub track: Track,
    pub disposition: DownloadDisposition,
    pub existing_path: Option<String>,
    pub reason: Option<String>,
}

impl LibraryDb {
    pub fn open_default() -> Result<Self> {
        let db_dir = crate::library::data_dir();
        let db_path = db_dir.join("kanono_x_library.db");
        Self::open(&db_path)
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let conn = Connection::open(path)?;
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let _ = conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;");

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                duration REAL NOT NULL,
                file_path TEXT,
                is_stream INTEGER NOT NULL,
                stream_url TEXT,
                support_url TEXT,
                track_number INTEGER,
                bitrate INTEGER,
                sample_rate INTEGER,
                play_count INTEGER DEFAULT 0,
                last_played INTEGER,
                added_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
            CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
            CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(file_path);
            CREATE TABLE IF NOT EXISTS library_folders (
                path TEXT PRIMARY KEY,
                added_at INTEGER NOT NULL,
                last_scanned INTEGER
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;

        for (name, definition) in [
            ("support_url", "TEXT"),
            ("track_number", "INTEGER"),
            ("bitrate", "INTEGER"),
            ("sample_rate", "INTEGER"),
            ("file_size", "INTEGER"),
            ("file_modified", "INTEGER"),
            ("norm_artist", "TEXT"),
            ("norm_title", "TEXT"),
            ("norm_album", "TEXT"),
            ("musicbrainz_recording_id", "TEXT"),
            ("isrc", "TEXT"),
            ("fingerprint", "TEXT"),
        ] {
            Self::ensure_column(&conn, name, definition)?;
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_tracks_norm_artist ON tracks(norm_artist);
             CREATE INDEX IF NOT EXISTS idx_tracks_isrc ON tracks(isrc);
             CREATE INDEX IF NOT EXISTS idx_tracks_mbid ON tracks(musicbrainz_recording_id);",
        )?;

        Ok(Self {
            conn,
            db_path: path.to_path_buf(),
        })
    }

    fn ensure_column(conn: &Connection, name: &str, definition: &str) -> Result<()> {
        let exists: Option<String> = conn
            .query_row(
                "SELECT name FROM pragma_table_info('tracks') WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            conn.execute(
                &format!("ALTER TABLE tracks ADD COLUMN {name} {definition}"),
                [],
            )?;
        }
        Ok(())
    }

    pub fn upsert_track(&mut self, track: &Track) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO tracks (id, title, artist, album, duration, file_path, is_stream, stream_url, support_url, track_number, bitrate, sample_rate, file_size, file_modified, norm_artist, norm_title, norm_album, musicbrainz_recording_id, isrc, fingerprint, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 artist = excluded.artist,
                 album = excluded.album,
                 duration = excluded.duration,
                 file_path = excluded.file_path,
                 is_stream = excluded.is_stream,
                 stream_url = excluded.stream_url,
                 support_url = excluded.support_url,
                 track_number = excluded.track_number,
                 bitrate = excluded.bitrate,
                 sample_rate = excluded.sample_rate,
                 file_size = excluded.file_size,
                 file_modified = excluded.file_modified,
                 norm_artist = excluded.norm_artist,
                 norm_title = excluded.norm_title,
                 norm_album = excluded.norm_album,
                 musicbrainz_recording_id = excluded.musicbrainz_recording_id,
                 isrc = excluded.isrc,
                 fingerprint = excluded.fingerprint",
            params![
                track.id,
                track.title,
                track.artist,
                track.album,
                track.duration,
                track.file_path,
                if track.is_stream { 1 } else { 0 },
                track.stream_url,
                track.support_url,
                track.track_number,
                track.bitrate,
                track.sample_rate,
                file_stamp(track.file_path.as_deref()),
                file_modified(track.file_path.as_deref()),
                normalize(&track.artist),
                normalize(&track.title),
                normalize(&track.album),
                track.musicbrainz_recording_id,
                track.isrc,
                track.fingerprint,
                now,
            ],
        )?;

        Ok(())
    }

    pub fn upsert_tracks_batch(&mut self, tracks: &[Track]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut inserted = 0;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO tracks (id, title, artist, album, duration, file_path, is_stream, stream_url, support_url, track_number, bitrate, sample_rate, file_size, file_modified, norm_artist, norm_title, norm_album, musicbrainz_recording_id, isrc, fingerprint, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                 ON CONFLICT(id) DO UPDATE SET
                     title = excluded.title,
                     artist = excluded.artist,
                     album = excluded.album,
                     duration = excluded.duration,
                     file_path = excluded.file_path,
                     is_stream = excluded.is_stream,
                     stream_url = excluded.stream_url,
                     support_url = excluded.support_url,
                     track_number = excluded.track_number,
                     bitrate = excluded.bitrate,
                     sample_rate = excluded.sample_rate,
                     file_size = excluded.file_size,
                     file_modified = excluded.file_modified,
                     norm_artist = excluded.norm_artist,
                     norm_title = excluded.norm_title,
                     norm_album = excluded.norm_album,
                     musicbrainz_recording_id = excluded.musicbrainz_recording_id,
                     isrc = excluded.isrc,
                     fingerprint = excluded.fingerprint",
            )?;

            for track in tracks {
                stmt.execute(params![
                    track.id,
                    track.title,
                    track.artist,
                    track.album,
                    track.duration,
                    track.file_path,
                    if track.is_stream { 1 } else { 0 },
                    track.stream_url,
                    track.support_url,
                    track.track_number,
                    track.bitrate,
                    track.sample_rate,
                    file_stamp(track.file_path.as_deref()),
                    file_modified(track.file_path.as_deref()),
                    normalize(&track.artist),
                    normalize(&track.title),
                    normalize(&track.album),
                    track.musicbrainz_recording_id,
                    track.isrc,
                    track.fingerprint,
                    now,
                ])?;
                inserted += 1;
            }
        }

        tx.commit()?;
        Ok(inserted)
    }

    pub fn load_all_tracks(&self) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, artist, album, duration, file_path, is_stream, stream_url, support_url, track_number, bitrate, sample_rate, musicbrainz_recording_id, isrc, fingerprint FROM tracks ORDER BY rowid ASC",
        )?;

        let track_iter = stmt.query_map([], |row| {
            let is_stream_num: i32 = row.get(6)?;
            Ok(Track {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                duration: row.get(4)?,
                file_path: row.get(5)?,
                is_stream: is_stream_num != 0,
                stream_url: row.get(7)?,
                support_url: row.get(8)?,
                track_number: row.get(9)?,
                bitrate: row.get(10)?,
                sample_rate: row.get(11)?,
                musicbrainz_recording_id: row.get(12)?,
                isrc: row.get(13)?,
                fingerprint: row.get(14)?,
            })
        })?;

        let mut tracks = Vec::new();
        for t in track_iter {
            if let Ok(track) = t {
                tracks.push(track);
            }
        }

        Ok(tracks)
    }

    pub fn delete_track(&mut self, track_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![track_id])?;
        Ok(())
    }

    pub fn clear_all_tracks(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM tracks", [])?;
        Ok(())
    }

    pub fn prune_missing_under(&mut self, roots: &[PathBuf]) -> Result<usize> {
        let paths: Vec<(String, String)> = {
            let mut statement = self.conn.prepare(
                "SELECT id, file_path FROM tracks WHERE is_stream = 0 AND file_path IS NOT NULL",
            )?;
            let rows = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .flatten()
                .collect();
            rows
        };
        let mut removed = 0;
        for (id, path) in paths {
            let path_buf = PathBuf::from(&path);
            let under_root = roots.iter().any(|root| path_buf.starts_with(root));
            if under_root && !path_buf.is_file() {
                removed += self
                    .conn
                    .execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
            }
        }
        Ok(removed)
    }

    pub fn record_play(&mut self, track_id: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
            params![now, track_id],
        )?;
        Ok(())
    }

    pub fn save_folder(&mut self, folder: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO library_folders (path, added_at, last_scanned) VALUES (?1, ?2, ?2)
             ON CONFLICT(path) DO UPDATE SET last_scanned = ?2",
            params![folder, now],
        )?;
        Ok(())
    }

    pub fn remove_folder(&mut self, folder: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM library_folders WHERE path = ?1",
            params![folder],
        )?;
        Ok(())
    }

    pub fn get_stats(&self) -> DbStats {
        let total_tracks: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total_artists: usize = self
            .conn
            .query_row("SELECT COUNT(DISTINCT artist) FROM tracks", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let total_albums: usize = self
            .conn
            .query_row("SELECT COUNT(DISTINCT album) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total_duration_secs: f32 = self
            .conn
            .query_row("SELECT COALESCE(SUM(duration), 0.0) FROM tracks", [], |r| {
                r.get(0)
            })
            .unwrap_or(0.0);

        let total_plays: u64 = self
            .conn
            .query_row("SELECT COALESCE(SUM(play_count), 0) FROM tracks", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        let db_file_size_bytes = fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0);

        DbStats {
            total_tracks,
            total_artists,
            total_albums,
            total_duration_secs,
            total_plays,
            db_file_size_bytes,
        }
    }

    pub fn vacuum(&mut self) -> Result<()> {
        self.conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn duplicate_policy(&self) -> DuplicatePolicy {
        self.get_setting("duplicate_policy")
            .map(|value| DuplicatePolicy::from_str(&value))
            .unwrap_or_default()
    }

    pub fn find_duplicates(&self, candidate: &DuplicateCandidate) -> Result<Vec<DuplicateMatch>> {
        let tracks = self.load_all_tracks()?;
        let mut matches = tracks
            .into_iter()
            .filter(|track| {
                track
                    .file_path
                    .as_deref()
                    .is_some_and(|path| Path::new(path).is_file())
            })
            .filter_map(|track| match_candidate(candidate, track))
            .collect::<Vec<_>>();
        matches.sort_by_key(|matched| match matched.reason {
            MatchReason::MusicBrainzRecordingId => 0,
            MatchReason::Isrc => 1,
            MatchReason::Fingerprint => 2,
            MatchReason::Metadata => 3,
        });
        Ok(matches)
    }
}

fn file_stamp(path: Option<&str>) -> Option<i64> {
    path.and_then(|value| fs::metadata(value).ok())
        .and_then(|metadata| metadata.len().try_into().ok())
}

fn file_modified(path: Option<&str>) -> Option<i64> {
    path.and_then(|value| fs::metadata(value).ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().try_into().unwrap_or(i64::MAX))
}

fn normalize(value: &str) -> String {
    value
        .nfkd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn version_markers(value: &str) -> Vec<&'static str> {
    const MARKERS: &[&str] = &[
        "acoustic",
        "demo",
        "live",
        "remaster",
        "remix",
        "edit",
        "instrumental",
        "karaoke",
        "radio",
        "session",
        "sped up",
        "slowed",
    ];
    let normalized = normalize(value);
    MARKERS
        .iter()
        .copied()
        .filter(|marker| normalized.contains(marker))
        .collect()
}

fn same_version(title_a: &str, title_b: &str) -> bool {
    version_markers(title_a) == version_markers(title_b)
}

fn match_candidate(candidate: &DuplicateCandidate, track: Track) -> Option<DuplicateMatch> {
    if let (Some(left), Some(right)) = (
        candidate.musicbrainz_recording_id.as_deref(),
        track.musicbrainz_recording_id.as_deref(),
    ) {
        if normalize(left) == normalize(right) {
            return Some(DuplicateMatch {
                track,
                reason: MatchReason::MusicBrainzRecordingId,
            });
        }
    }
    if let (Some(left), Some(right)) = (candidate.isrc.as_deref(), track.isrc.as_deref()) {
        if normalize(left) == normalize(right) {
            return Some(DuplicateMatch {
                track,
                reason: MatchReason::Isrc,
            });
        }
    }
    if let (Some(left), Some(right)) = (
        candidate.fingerprint.as_deref(),
        track.fingerprint.as_deref(),
    ) {
        if left == right {
            return Some(DuplicateMatch {
                track,
                reason: MatchReason::Fingerprint,
            });
        }
    }

    if candidate.duration <= 0.0 || track.duration <= 0.0 {
        return None;
    }
    let duration_delta = (candidate.duration - track.duration).abs();
    let duration_tolerance = candidate.duration.max(track.duration) * 0.03;
    if normalize(&candidate.artist) == normalize(&track.artist)
        && title_key(&candidate.title) == title_key(&track.title)
        && same_version(&candidate.title, &track.title)
        && duration_delta <= duration_tolerance.max(2.0)
    {
        let album_conflicts = candidate.album.as_deref().is_some_and(|album| {
            !normalize(album).is_empty() && normalize(album) != normalize(&track.album)
        });
        let track_number_conflicts = candidate
            .track_number
            .is_some_and(|number| track.track_number != Some(number));
        if !album_conflicts && !track_number_conflicts {
            return Some(DuplicateMatch {
                track,
                reason: MatchReason::Metadata,
            });
        }
    }
    None
}

fn title_key(value: &str) -> String {
    normalize(value)
        .split_whitespace()
        .filter(|word| !matches!(*word, "official" | "audio" | "video" | "lyrics"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, duration: f32) -> Track {
        Track {
            id: "test".to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: "Album".to_string(),
            duration,
            file_path: Some("/tmp/test.mp3".to_string()),
            is_stream: false,
            stream_url: None,
            support_url: None,
            track_number: None,
            bitrate: None,
            sample_rate: None,
            musicbrainz_recording_id: None,
            isrc: None,
            fingerprint: None,
        }
    }

    #[test]
    fn normalizes_unicode_punctuation_and_case() {
        assert_eq!(normalize("Beyoncé — Déjà Vu!"), "beyonce deja vu");
    }

    #[test]
    fn does_not_match_conflicting_recording_versions() {
        let candidate = DuplicateCandidate {
            title: "Song (Live)".to_string(),
            artist: "Artist".to_string(),
            duration: 180.0,
            ..Default::default()
        };
        assert!(match_candidate(&candidate, track("Song (Remix)", "Artist", 180.0)).is_none());
    }

    #[test]
    fn matches_strict_metadata_with_duration_tolerance() {
        let candidate = DuplicateCandidate {
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            duration: 180.0,
            ..Default::default()
        };
        assert_eq!(
            match_candidate(&candidate, track("Song", "Artist", 181.0))
                .expect("metadata should match")
                .reason,
            MatchReason::Metadata
        );
        assert!(match_candidate(&candidate, track("Song", "Artist", 190.0)).is_none());
    }
}
