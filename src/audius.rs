use crate::library::Track;
use crate::library_db::{
    DownloadDisposition, DownloadResult, DuplicateCandidate, DuplicatePolicy, LibraryDb,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const FALLBACK_API_HOST: &str = "https://api.audius.co";

#[derive(Clone, Debug, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiTrack {
    id: String,
    title: String,
    #[serde(default)]
    duration: f32,
    #[serde(default)]
    is_downloadable: bool,
    #[serde(default)]
    user: Option<ApiUser>,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiUser {
    #[serde(default)]
    name: String,
    #[serde(default)]
    handle: String,
    #[serde(default)]
    permalink: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AudiusItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f32,
    pub is_downloadable: bool,
    pub stream_url: String,
    pub support_url: Option<String>,
}

impl AudiusItem {
    pub fn duration_formatted(&self) -> String {
        crate::library::format_time(self.duration)
    }

    pub fn to_track(&self, file_path: Option<String>) -> Track {
        let is_stream = file_path.is_none();
        Track {
            id: format!("audius:{}", self.id),
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration: self.duration,
            file_path,
            is_stream,
            stream_url: Some(self.stream_url.clone()),
            support_url: self.support_url.clone(),
            track_number: None,
            bitrate: None,
            sample_rate: None,
            musicbrainz_recording_id: None,
            isrc: None,
            fingerprint: None,
        }
    }
}

pub fn search(query: &str) -> Result<Vec<AudiusItem>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("kanono-x/0.2.0")
        .build()
        .map_err(|e| format!("Could not create Audius client: {}", e))?;
    let resolved_host = resolve_api_host(&client);
    let hosts = if resolved_host == FALLBACK_API_HOST {
        vec![resolved_host]
    } else {
        vec![resolved_host, FALLBACK_API_HOST.to_string()]
    };
    let mut last_error = "no Audius API host responded".to_string();
    for host in hosts {
        let response = client
            .get(format!("{host}/v1/tracks/search"))
            .query(&[("query", query), ("limit", "20")])
            .send();
        match response {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<ApiResponse<Vec<ApiTrack>>>() {
                    Ok(response) => {
                        return Ok(response
                            .data
                            .into_iter()
                            .map(|track| to_item(&host, track))
                            .collect());
                    }
                    Err(error) => {
                        last_error = format!("Audius returned invalid search data: {error}")
                    }
                },
                Err(error) => last_error = format!("Audius search returned an error: {error}"),
            },
            Err(error) => last_error = format!("Audius search failed: {error}"),
        }
    }
    Err(last_error)
}

pub fn stream_to_cache(item: &AudiusItem) -> Result<Track, String> {
    let path = stream_path(&item.id);
    download_bytes(&item.stream_url, &path)?;
    Ok(item.to_track(Some(path.to_string_lossy().to_string())))
}

pub fn download_to_library(
    item: &AudiusItem,
    library_dir: &Path,
) -> Result<DownloadResult, String> {
    if !item.is_downloadable {
        return Err("The artist has not marked this track as downloadable".to_string());
    }
    fs::create_dir_all(library_dir).map_err(|e| format!("Could not create music folder: {}", e))?;
    let db =
        LibraryDb::open_default().map_err(|e| format!("Could not open library index: {}", e))?;
    let matches = db
        .find_duplicates(&DuplicateCandidate {
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: Some(item.album.clone()),
            duration: item.duration,
            ..Default::default()
        })
        .map_err(|e| format!("Could not query library index: {}", e))?;
    let existing = match matches.as_slice() {
        [] => None,
        [single] => Some(single.clone()),
        _ => {
            return Err(format!(
                "Ambiguous existing matches for '{}'; no download was started",
                item.title
            ))
        }
    };
    match (db.duplicate_policy(), existing.as_ref()) {
        (DuplicatePolicy::Skip, Some(existing)) => {
            return Ok(DownloadResult {
                track: existing.track.clone(),
                disposition: DownloadDisposition::Reused,
                existing_path: existing.track.file_path.clone(),
                reason: Some(existing.reason.label().to_string()),
            });
        }
        (DuplicatePolicy::Ask, Some(existing)) => {
            return Err(format!(
                "Existing track found at {}; change Duplicate policy to Allow or Replace to continue",
                existing
                    .track
                    .file_path
                    .as_deref()
                    .unwrap_or("unknown path")
            ));
        }
        _ => {}
    }
    let album = if item.album.trim().is_empty() || item.album.eq_ignore_ascii_case("audius") {
        "Singles"
    } else {
        item.album.as_str()
    };
    let album_dir = library_dir
        .join(sanitize_component(&item.artist, "Unknown Artist"))
        .join(sanitize_component(album, "Singles"));
    fs::create_dir_all(&album_dir).map_err(|e| format!("Could not create album folder: {}", e))?;
    let filename = format!("{}.mp3", sanitize(&item.title));
    let path = if db.duplicate_policy() == DuplicatePolicy::Replace {
        existing
            .as_ref()
            .and_then(|matched| matched.track.file_path.as_deref())
            .map(PathBuf::from)
            .unwrap_or_else(|| unique_path(&album_dir, &filename))
    } else {
        unique_path(&album_dir, &filename)
    };
    download_bytes(&item.stream_url, &path)?;
    let mut track = crate::library::load_local_track(&path)
        .ok_or_else(|| "Downloaded track could not be parsed".to_string())?;
    track.support_url = item.support_url.clone();
    track.album = album.to_string();
    Ok(DownloadResult {
        track,
        disposition: DownloadDisposition::Downloaded,
        existing_path: existing.and_then(|matched| matched.track.file_path),
        reason: None,
    })
}

fn resolve_api_host(client: &reqwest::blocking::Client) -> String {
    client
        .get("https://audius.co")
        .send()
        .ok()
        .map(|response| {
            let url = response.url();
            format!(
                "{}://{}",
                url.scheme(),
                url.host_str().unwrap_or("audius.co")
            )
        })
        .filter(|host| host.starts_with("https://") && host.contains("audius.co"))
        .unwrap_or_else(|| FALLBACK_API_HOST.to_string())
}

fn to_item(host: &str, track: ApiTrack) -> AudiusItem {
    let (artist, support_url) = track
        .user
        .map(|user| {
            let artist = if user.name.is_empty() {
                user.handle
            } else {
                user.name
            };
            (artist, user.permalink.filter(|url| is_http_url(url)))
        })
        .unwrap_or_else(|| ("Unknown Artist".to_string(), None));
    AudiusItem {
        stream_url: format!("{host}/v1/tracks/{}/stream", track.id),
        id: track.id,
        title: track.title,
        artist,
        album: "Audius".to_string(),
        duration: track.duration,
        is_downloadable: track.is_downloadable,
        support_url,
    }
}

fn stream_path(id: &str) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("kanono-x")
        .join("audius")
        .join(format!("{}.mp3", sanitize(id)))
}

fn download_bytes(url: &str, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create cache folder: {}", e))?;
    }
    let bytes = reqwest::blocking::get(url)
        .map_err(|e| format!("Audius stream request failed: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Audius stream returned an error: {}", e))?
        .bytes()
        .map_err(|e| format!("Could not read Audius stream: {}", e))?;
    fs::write(path, bytes).map_err(|e| format!("Could not save audio data: {}", e))
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let initial = dir.join(filename);
    if !initial.exists() {
        return initial;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("track");
    let ext = Path::new(filename)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("mp3");
    for index in 2.. {
        let candidate = dir.join(format!("{stem} ({index}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn sanitize(value: &str) -> String {
    let output: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if output.trim().is_empty() {
        "track".to_string()
    } else {
        output
    }
}

fn sanitize_component(value: &str, fallback: &str) -> String {
    let output: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, ' ' | '-' | '_' | '(' | ')' | '[' | ']')
            {
                character
            } else {
                '_'
            }
        })
        .collect();
    let output = output.trim();
    if output.is_empty() {
        fallback.to_string()
    } else {
        output.to_string()
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
