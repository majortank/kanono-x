use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::library::{extract_metadata, Track};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct YouTubeItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: f32,
    pub url: String,
}

impl YouTubeItem {
    pub fn duration_formatted(&self) -> String {
        crate::library::format_time(self.duration)
    }
}

pub fn search_or_resolve(query: &str) -> Result<Vec<YouTubeItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let search_target = if query.contains("youtube.com") || query.contains("youtu.be") {
        query.to_string()
    } else {
        format!("ytsearch5:{}", query)
    };

    let output = Command::new("yt-dlp")
        .args([
            "--print",
            "%(id)s\t%(title)s\t%(uploader,channel)s\t%(duration)s",
            "--no-playlist",
            &search_target,
        ])
        .output()
        .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp error: {}", err));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let id = parts[0].trim().to_string();
            let title = parts[1].trim().to_string();
            let channel = parts[2].trim().to_string();
            let duration = parts[3].trim().parse::<f32>().unwrap_or(0.0);
            let url = format!("https://www.youtube.com/watch?v={}", id);

            if !id.is_empty() && !title.is_empty() {
                results.push(YouTubeItem {
                    id,
                    title,
                    channel: if channel.is_empty() { "YouTube Artist".to_string() } else { channel },
                    duration,
                    url,
                });
            }
        }
    }

    Ok(results)
}

pub fn download_and_convert_mp3(url_or_id: &str, target_dir: &Path) -> Result<Track, String> {
    let _ = fs::create_dir_all(target_dir);
    let target_template = target_dir.join("%(title)s.%(ext)s");
    let template_str = target_template.to_string_lossy().to_string();

    let full_url = if url_or_id.starts_with("http://") || url_or_id.starts_with("https://") {
        url_or_id.to_string()
    } else {
        format!("https://www.youtube.com/watch?v={}", url_or_id)
    };

    let output = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "--embed-metadata",
            "-o",
            &template_str,
            "--print",
            "after_move:filepath",
            &full_url,
        ])
        .output()
        .map_err(|e| format!("Failed to run yt-dlp converter: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Conversion failed: {}", err));
    }

    let filepath_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mp3_path = if !filepath_str.is_empty() && Path::new(&filepath_str).exists() {
        PathBuf::from(&filepath_str)
    } else {
        // Fallback: look for newly created mp3 in target_dir
        let mut found = None;
        if let Ok(entries) = fs::read_dir(target_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map_or(false, |ext| ext == "mp3") {
                    found = Some(p);
                }
            }
        }
        found.ok_or_else(|| "Could not locate converted MP3 file".to_string())?
    };

    crate::library::load_local_track(&mp3_path).ok_or_else(|| "Converted file could not be read".to_string())
}

pub fn prepare_stream_cache(url_or_id: &str) -> Result<Track, String> {
    let cache_dir = std::env::temp_dir().join("kanono_stream_cache");
    let _ = fs::create_dir_all(&cache_dir);
    let target_template = cache_dir.join("%(id)s.%(ext)s");
    let template_str = target_template.to_string_lossy().to_string();

    let full_url = if url_or_id.starts_with("http://") || url_or_id.starts_with("https://") {
        url_or_id.to_string()
    } else {
        format!("https://www.youtube.com/watch?v={}", url_or_id)
    };

    let output = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "3",
            "-o",
            &template_str,
            "--print",
            "after_move:filepath",
            &full_url,
        ])
        .output()
        .map_err(|e| format!("Failed to stream YouTube audio: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Streaming audio buffer failed: {}", err));
    }

    let filepath_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mp3_path = PathBuf::from(&filepath_str);
    if !mp3_path.exists() {
        return Err("Stream cache file not found".to_string());
    }

    let (title, artist, album, duration) = extract_metadata(&mp3_path);
    Ok(Track {
        id: format!("stream_{}", mp3_path.file_name().unwrap_or_default().to_string_lossy()),
        title,
        artist: format!("{} (Streamed)", artist),
        album,
        duration,
        file_path: Some(mp3_path.to_string_lossy().to_string()),
        is_stream: true,
        stream_url: Some(full_url),
    })
}
