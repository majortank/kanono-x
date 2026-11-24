use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use rusqlite::{params, Connection, Result};

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

impl LibraryDb {
    pub fn open_default() -> Result<Self> {
        let db_dir = dirs_or_local();
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

        Ok(Self {
            conn,
            db_path: path.to_path_buf(),
        })
    }

    pub fn upsert_track(&mut self, track: &Track) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO tracks (id, title, artist, album, duration, file_path, is_stream, stream_url, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title,
                 artist = excluded.artist,
                 album = excluded.album,
                 duration = excluded.duration,
                 file_path = excluded.file_path,
                 is_stream = excluded.is_stream,
                 stream_url = excluded.stream_url",
            params![
                track.id,
                track.title,
                track.artist,
                track.album,
                track.duration,
                track.file_path,
                if track.is_stream { 1 } else { 0 },
                track.stream_url,
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
                "INSERT INTO tracks (id, title, artist, album, duration, file_path, is_stream, stream_url, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     title = excluded.title,
                     artist = excluded.artist,
                     album = excluded.album,
                     duration = excluded.duration,
                     file_path = excluded.file_path,
                     is_stream = excluded.is_stream,
                     stream_url = excluded.stream_url",
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
            "SELECT id, title, artist, album, duration, file_path, is_stream, stream_url FROM tracks ORDER BY rowid ASC",
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
        self.conn.execute("DELETE FROM tracks WHERE id = ?1", params![track_id])?;
        Ok(())
    }

    pub fn clear_all_tracks(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM tracks", [])?;
        Ok(())
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
        self.conn.execute("DELETE FROM library_folders WHERE path = ?1", params![folder])?;
        Ok(())
    }

    pub fn load_saved_folders(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT path FROM library_folders ORDER BY added_at ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut folders = Vec::new();
        for r in rows {
            if let Ok(f) = r {
                folders.push(f);
            }
        }
        Ok(folders)
    }

    pub fn get_stats(&self) -> DbStats {
        let total_tracks: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total_artists: usize = self
            .conn
            .query_row("SELECT COUNT(DISTINCT artist) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total_albums: usize = self
            .conn
            .query_row("SELECT COUNT(DISTINCT album) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let total_duration_secs: f32 = self
            .conn
            .query_row("SELECT COALESCE(SUM(duration), 0.0) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0.0);

        let total_plays: u64 = self
            .conn
            .query_row("SELECT COALESCE(SUM(play_count), 0) FROM tracks", [], |r| r.get(0))
            .unwrap_or(0);

        let db_file_size_bytes = fs::metadata(&self.db_path)
            .map(|m| m.len())
            .unwrap_or(0);

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
            .query_row("SELECT value FROM app_settings WHERE key = ?1", params![key], |r| r.get(0))
            .ok()
    }
}

fn dirs_or_local() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/share/kanono-x");
        if fs::create_dir_all(&p).is_ok() {
            return p;
        }
    }
    PathBuf::from(".")
}
