use lofty::prelude::{AudioFile, ItemKey, TaggedFileExt};
use lofty::read_from_path;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f32,
    pub file_path: Option<String>,
    pub is_stream: bool,
    pub stream_url: Option<String>,
    #[serde(default)]
    pub support_url: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub bitrate: Option<u32>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub musicbrainz_recording_id: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f32,
    pub track_number: Option<u32>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub support_url: Option<String>,
    #[serde(default)]
    pub musicbrainz_recording_id: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CacheEntry {
    size: u64,
    modified: u64,
    metadata: Metadata,
}

static META_CACHE: Mutex<Option<HashMap<String, CacheEntry>>> = Mutex::new(None);

pub fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join("kanono-x");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn cache_path() -> PathBuf {
    data_dir().join("library_cache.json")
}

fn state_path() -> PathBuf {
    data_dir().join("library_state.json")
}

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
                let _ = fs::write(cache_path(), text);
            }
        }
    }
}

pub fn extract_metadata(file_path: &Path) -> Metadata {
    let key = file_path.to_string_lossy().to_string();
    let (size, modified) = file_stamp(file_path);
    if let Ok(mut guard) = META_CACHE.lock() {
        let map = guard.get_or_insert_with(|| {
            fs::read_to_string(cache_path())
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        });
        if let Some(entry) = map.get(&key) {
            if entry.size == size && entry.modified == modified {
                return entry.metadata.clone();
            }
        }
    }

    let metadata = read_metadata(file_path);
    if let Ok(mut guard) = META_CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            map.insert(
                key,
                CacheEntry {
                    size,
                    modified,
                    metadata: metadata.clone(),
                },
            );
        }
    }
    metadata
}

fn read_metadata(file_path: &Path) -> Metadata {
    let default_title = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Track")
        .to_string();
    let mut metadata = Metadata {
        title: default_title,
        artist: "Unknown Artist".to_string(),
        album: "Unknown Album".to_string(),
        duration: 0.0,
        track_number: None,
        bitrate: None,
        sample_rate: None,
        support_url: None,
        musicbrainz_recording_id: None,
        isrc: None,
        fingerprint: None,
    };

    let Ok(tagged_file) = read_from_path(file_path) else {
        return metadata;
    };

    let properties = tagged_file.properties();
    metadata.duration = properties.duration().as_secs_f32();
    metadata.bitrate = properties.audio_bitrate();
    metadata.sample_rate = properties.sample_rate();

    if let Some(tag) = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
    {
        if let Some(value) = tag.get_string(ItemKey::TrackTitle) {
            if !value.is_empty() {
                metadata.title = value.to_string();
            }
        }
        if let Some(value) = tag.get_string(ItemKey::TrackArtist) {
            if !value.is_empty() {
                metadata.artist = value.to_string();
            }
        }
        if let Some(value) = tag.get_string(ItemKey::AlbumTitle) {
            if !value.is_empty() {
                metadata.album = value.to_string();
            }
        }
        metadata.track_number = tag
            .get_string(ItemKey::TrackNumber)
            .and_then(|value| value.split('/').next()?.parse().ok());
        metadata.support_url = tag
            .items()
            .map(|item| format!("{:?}", item.value()))
            .find_map(|value| find_http_url(&value));
        metadata.musicbrainz_recording_id = tag
            .get_string(ItemKey::MusicBrainzRecordingId)
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty());
        metadata.isrc = tag
            .get_string(ItemKey::Isrc)
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty());
    }

    metadata.fingerprint = audio_fingerprint(file_path);
    metadata
}

fn audio_fingerprint(file_path: &Path) -> Option<String> {
    let output = std::process::Command::new("fpcalc")
        .args(["-json", "-length", "120"])
        .arg(file_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .ok()?
        .get("fingerprint")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn find_http_url(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .map(|candidate| candidate.trim_matches(|c: char| "\"'<>[](),".contains(c)))
        .find(|candidate| candidate.starts_with("https://") || candidate.starts_with("http://"))
        .map(ToString::to_string)
}

pub fn load_local_track(file_path: &Path) -> Option<Track> {
    if !file_path.exists() || !file_path.is_file() {
        return None;
    }
    let metadata = extract_metadata(file_path);
    let id = format!("{:x}", md5_hash(file_path.to_str().unwrap_or_default()));
    Some(Track {
        id,
        title: metadata.title,
        artist: metadata.artist,
        album: metadata.album,
        duration: metadata.duration,
        file_path: Some(file_path.to_string_lossy().to_string()),
        is_stream: false,
        stream_url: None,
        support_url: metadata.support_url,
        track_number: metadata.track_number,
        bitrate: metadata.bitrate,
        sample_rate: metadata.sample_rate,
        musicbrainz_recording_id: metadata.musicbrainz_recording_id,
        isrc: metadata.isrc,
        fingerprint: metadata.fingerprint,
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
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
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

pub fn scan_directory(dir: &Path) -> Vec<Track> {
    let mut files = Vec::new();
    collect_files(dir, 0, &mut files);
    files.sort();
    let mut tracks: Vec<Track> = files.iter().filter_map(|p| load_local_track(p)).collect();
    tracks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    tracks
}

pub fn ensure_library_dir() -> PathBuf {
    let library_dir = dirs::audio_dir()
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| data_dir().join("library"));
    let _ = fs::create_dir_all(&library_dir);
    library_dir
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct SavedLibrary {
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub files: Vec<String>,
}

pub fn load_saved() -> SavedLibrary {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_saved(s: &SavedLibrary) {
    if let Ok(text) = serde_json::to_string_pretty(s) {
        let _ = fs::write(state_path(), text);
    }
}

fn md5_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}
