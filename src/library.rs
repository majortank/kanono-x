use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f32, // Duration in seconds
    pub file_path: Option<String>,
    pub is_stream: bool,
    pub stream_url: Option<String>,
}

impl Track {
    pub fn duration_formatted(&self) -> String {
        format_time(self.duration)
    }
}

pub fn format_time(seconds: f32) -> String {
    if seconds.is_nan() || seconds < 0.0 {
        return "00:00".to_string();
    }
    let total_secs = seconds.round() as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    let hours = mins / 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins % 60, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

type Meta = (String, String, String, f32);

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CacheEntry {
    size: u64,
    modified: u64,
    meta: Meta,
}

static META_CACHE: Mutex<Option<HashMap<String, CacheEntry>>> = Mutex::new(None);
const CACHE_FILE: &str = "library_cache.json";

fn file_stamp(path: &Path) -> (u64, u64) {
    let md = fs::metadata(path).ok();
    let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
    let modified = md
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (size, modified)
}

pub fn flush_cache() {
    if let Ok(guard) = META_CACHE.lock() {
        if let Some(map) = guard.as_ref() {
            if let Ok(text) = serde_json::to_string(map) {
                let _ = fs::write(CACHE_FILE, text);
            }
        }
    }
}

pub fn extract_metadata(file_path: &Path) -> Meta {
    let key = file_path.to_string_lossy().to_string();
    let (size, modified) = file_stamp(file_path);
    if let Ok(mut guard) = META_CACHE.lock() {
        let map = guard.get_or_insert_with(|| {
            fs::read_to_string(CACHE_FILE)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        });
        if let Some(e) = map.get(&key) {
            if e.size == size && e.modified == modified {
                return e.meta.clone();
            }
        }
    }
    let meta = probe_metadata(file_path);
    if let Ok(mut guard) = META_CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            map.insert(key, CacheEntry { size, modified, meta: meta.clone() });
        }
    }
    meta
}

fn probe_metadata(file_path: &Path) -> Meta {
    let default_title = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Track")
        .to_string();
    let mut title = default_title.clone();
    let mut artist = "Unknown Artist".to_string();
    let mut album = "Unknown Album".to_string();
    let mut duration = 0.0f32;

    // Run ffprobe to extract tags and duration
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:format_tags=title,artist,album",
            "-of",
            "default=noprint_wrappers=1",
            file_path.to_str().unwrap_or_default(),
        ])
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let line = line.trim();
            if let Some(dur_str) = line.strip_prefix("duration=") {
                if let Ok(d) = dur_str.parse::<f32>() {
                    if d.is_finite() && d >= 0.0 {
                        duration = d;
                    }
                }
            } else if let Some(t) = line.strip_prefix("TAG:title=") {
                if !t.is_empty() {
                    title = t.to_string();
                }
            } else if let Some(a) = line.strip_prefix("TAG:artist=") {
                if !a.is_empty() {
                    artist = a.to_string();
                }
            } else if let Some(al) = line.strip_prefix("TAG:album=") {
                if !al.is_empty() {
                    album = al.to_string();
                }
            }
        }
    }

    (title, artist, album, duration)
}

pub fn load_local_track(file_path: &Path) -> Option<Track> {
    if !file_path.exists() || !file_path.is_file() {
        return None;
    }
    let (title, artist, album, duration) = extract_metadata(file_path);
    let id = format!("{:x}", md5_hash(file_path.to_str().unwrap_or_default()));
    Some(Track {
        id,
        title,
        artist,
        album,
        duration,
        file_path: Some(file_path.to_string_lossy().to_string()),
        is_stream: false,
        stream_url: None,
    })
}

impl Track {
    pub fn folder(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Streams".to_string())
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| Path::new(p).file_name())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus"
            )
        })
        .unwrap_or(false)
}

fn collect_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 12 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_symlink = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        if path.is_dir() {
            if !is_symlink {
                collect_files(&path, depth + 1, out);
            }
        } else if path.is_file() && is_audio(&path) {
            out.push(path);
        }
    }
}

/// Scans a folder and all its subfolders for audio files.
pub fn scan_directory(dir: &Path) -> Vec<Track> {
    let mut files = Vec::new();
    collect_files(dir, 0, &mut files);
    files.sort();
    let mut tracks: Vec<Track> = files.iter().filter_map(|p| load_local_track(p)).collect();
    tracks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    tracks
}

pub fn ensure_library_dirs() -> (PathBuf, PathBuf) {
    let demo_dir = PathBuf::from("demo_tracks");
    let lib_dir = PathBuf::from("library");
    let _ = fs::create_dir_all(&demo_dir);
    let _ = fs::create_dir_all(&lib_dir);
    (demo_dir, lib_dir)
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct SavedLibrary {
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

const STATE_FILE: &str = "library_state.json";

pub fn load_saved() -> SavedLibrary {
    fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_saved(s: &SavedLibrary) {
    if let Ok(text) = serde_json::to_string_pretty(s) {
        let _ = fs::write(STATE_FILE, text);
    }
}

fn md5_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}
