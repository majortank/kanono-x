use crate::library::{load_local_track, Track};
use crate::library_db::{
    DownloadDisposition, DownloadResult, DuplicateCandidate, DuplicatePolicy, LibraryDb,
};
use serde::{Deserialize, Serialize};
use shell_words::split;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG_FILE: &str = "providers.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Provider {
    pub name: String,
    #[serde(default)]
    pub search_cmd: Option<String>,
    #[serde(default)]
    pub stream_cmd: Option<String>,
    #[serde(default)]
    pub download_cmd: Option<String>,
}

fn is_meaningful_metadata(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "na" | "n/a" | "none" | "unknown"
    )
}

fn organized_path(
    output_dir: &Path,
    artist: &str,
    album: &str,
    filename: &str,
    unique: bool,
) -> PathBuf {
    let artist = sanitize_component(artist, "Unknown Artist");
    let album = sanitize_component(album, "Singles");
    let dir = output_dir.join(artist).join(album);
    let _ = fs::create_dir_all(&dir);
    if unique {
        unique_path(&dir, filename)
    } else {
        dir.join(filename)
    }
}

fn sanitize_component(value: &str, fallback: &str) -> String {
    let sanitized: String = value
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
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProviderConfig {
    #[serde(default)]
    providers: Vec<Provider>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub duration: f32,
    pub url: String,
    #[serde(default)]
    pub downloadable: bool,
    #[serde(default)]
    pub support_url: Option<String>,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub musicbrainz_recording_id: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

impl ProviderItem {
    pub fn duration_formatted(&self) -> String {
        crate::library::format_time(self.duration)
    }

    pub fn to_track(&self, file_path: Option<String>, stream_url: Option<String>) -> Track {
        Track {
            id: format!("provider:{}:{}", self.provider, self.id),
            title: self.title.clone(),
            artist: if self.artist.is_empty() {
                "Unknown Artist".to_string()
            } else {
                self.artist.clone()
            },
            album: self.album.clone().unwrap_or_else(|| "Singles".to_string()),
            duration: self.duration,
            file_path,
            is_stream: stream_url.is_some(),
            stream_url,
            support_url: self.support_url.clone(),
            track_number: self.track_number,
            bitrate: None,
            sample_rate: None,
            musicbrainz_recording_id: self.musicbrainz_recording_id.clone(),
            isrc: self.isrc.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}

pub fn config_path() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("kanono-x");
    base.join(CONFIG_FILE)
}

pub fn load_configured_providers() -> Result<Vec<Provider>, String> {
    let path = config_path();
    if !path.exists() {
        write_template(&path)?;
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {}", path.display(), e))?;
    let config: ProviderConfig =
        toml::from_str(&text).map_err(|e| format!("Invalid provider configuration: {}", e))?;
    Ok(config.providers)
}

pub fn save_configured_providers(providers: &[Provider]) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {}", parent.display(), e))?;
    }
    let config = ProviderConfig {
        providers: providers.to_vec(),
    };
    let text = toml::to_string_pretty(&config)
        .map_err(|e| format!("Could not serialize provider configuration: {}", e))?;
    fs::write(&path, text).map_err(|e| format!("Could not write {}: {}", path.display(), e))
}

pub fn check_command(template: &str) -> Result<String, String> {
    let parts = split(template).map_err(|e| format!("Invalid command: {}", e))?;
    let program = parts
        .first()
        .ok_or_else(|| "Command is empty".to_string())?;
    let output = Command::new(program)
        .arg("--version")
        .output()
        .map_err(|e| format!("{} is not available: {}", program, e))?;
    if !output.status.success() {
        return Err(format!("{} did not accept --version", program));
    }
    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&output.stderr).lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("installed")
        .trim()
        .to_string();
    Ok(version)
}

pub fn installation_commands() -> &'static [(&'static str, &'static str)] {
    &[
        ("Debian / Ubuntu", "sudo apt install yt-dlp ffmpeg"),
        ("Arch Linux", "sudo pacman -S yt-dlp ffmpeg"),
        (
            "Python user install",
            "python3 -m pip install --user yt-dlp",
        ),
    ]
}

pub struct ProviderExample {
    pub name: &'static str,
    pub description: &'static str,
    pub provider: Provider,
}

pub fn examples() -> Vec<ProviderExample> {
    vec![
        ProviderExample {
            name: "yt-dlp + ffmpeg",
            description: "Search and stream with yt-dlp. Downloads stay disabled until you explicitly mark a result as permitted.",
            provider: Provider {
                name: "yt-dlp".to_string(),
                search_cmd: Some("yt-dlp \"ytsearch5:{query}\" --no-warnings --no-playlist --flat-playlist --print \"%(id)s\\t%(title)s\\t%(uploader,channel)s\\t%(duration)s\\t%(webpage_url)s\\ttrue\\t%(uploader_url)s\"".to_string()),
                stream_cmd: Some("yt-dlp --get-url --format bestaudio --no-playlist {url}".to_string()),
                download_cmd: Some("yt-dlp --extract-audio --audio-format mp3 --output {output_dir}/%(title)s.%(ext)s {url}".to_string()),
            },
        },
        ProviderExample {
            name: "Generic CLI provider",
            description: "A template for a tool that supports search, streaming, and downloading.",
            provider: Provider {
                name: "My Provider".to_string(),
                search_cmd: Some("my-provider search --query {query}".to_string()),
                stream_cmd: Some("my-provider stream --url {url}".to_string()),
                download_cmd: Some("my-provider download --url {url} --output-dir {output_dir}".to_string()),
            },
        },
    ]
}

fn write_template(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {}", parent.display(), e))?;
    }
    let template = r#"# Kanono X external provider configuration.
# Providers are opt-in and run only commands you configure locally.
#
# Search commands should print one result per line as tab-separated fields:
# id<TAB>title<TAB>artist<TAB>duration_seconds<TAB>url<TAB>downloadable<TAB>support_url
# They may also print one JSON object per line with the same field names.
# Supported placeholders are replaced with shell-escaped values:
#   {query}, {url}, {output_dir}
#
# [[providers]]
# name = "Example Provider"
# search_cmd = "provider-search {query}"
# stream_cmd = "provider-stream {url}"
# download_cmd = "provider-download {url} {output_dir}"
"#;
    fs::write(path, template).map_err(|e| format!("Could not write {}: {}", path.display(), e))
}

pub fn search(provider: &Provider, query: &str) -> Result<Vec<ProviderItem>, String> {
    let output = if provider.name.eq_ignore_ascii_case("yt-dlp") {
        run_yt_dlp_search(provider, query)?
    } else {
        let command = provider
            .search_cmd
            .as_deref()
            .ok_or_else(|| format!("Provider '{}' has no search command", provider.name))?;
        run_command(command, &[("{query}", query)])?
    };
    parse_search_output(provider, &output)
}

fn parse_search_output(provider: &Provider, output: &str) -> Result<Vec<ProviderItem>, String> {
    let mut items = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(mut item) = serde_json::from_str::<ProviderItem>(line) {
            item.provider = provider.name.clone();
            items.push(item);
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 5 {
            continue;
        }
        items.push(ProviderItem {
            id: fields[0].trim().to_string(),
            title: fields[1].trim().to_string(),
            artist: fields[2].trim().to_string(),
            duration: fields[3].trim().parse().unwrap_or_default(),
            url: fields[4].trim().to_string(),
            downloadable: fields
                .get(5)
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true")),
            support_url: fields
                .get(6)
                .map(|v| v.trim().to_string())
                .filter(|v| is_http_url(v)),
            provider: provider.name.clone(),
            album: fields
                .get(7)
                .map(|value| value.trim().to_string())
                .filter(|value| is_meaningful_metadata(value))
                .or_else(|| {
                    fields
                        .get(8)
                        .map(|value| value.trim().to_string())
                        .filter(|value| is_meaningful_metadata(value))
                }),
            track_number: fields.get(9).and_then(|value| value.trim().parse().ok()),
            musicbrainz_recording_id: None,
            isrc: None,
            fingerprint: None,
        });
    }
    Ok(items)
}

fn run_yt_dlp_search(provider: &Provider, query: &str) -> Result<String, String> {
    let command = provider
        .search_cmd
        .as_deref()
        .ok_or_else(|| format!("Provider '{}' has no search command", provider.name))?;
    let program = split(command)
        .map_err(|e| format!("Invalid yt-dlp provider command: {}", e))?
        .into_iter()
        .next()
        .ok_or_else(|| "yt-dlp provider command is empty".to_string())?;
    let is_url = is_youtube_url(query);
    let target = if is_url {
        query.trim().to_owned()
    } else {
        format!("ytsearch5:{}", query.trim())
    };
    let format = "%(id)s\t%(title)s\t%(uploader,channel)s\t%(duration)s\t%(webpage_url)s\ttrue\t%(uploader_url)s\t%(album)s\t%(playlist)s\t%(playlist_index)s";
    let mut args = vec!["--no-warnings", "--flat-playlist", "--print", format];
    if !is_url {
        args.push("--no-playlist");
    }
    let output = Command::new(&program)
        .args(args)
        .arg(target)
        .output()
        .map_err(|e| format!("Could not start provider command '{}': {}", program, e))?;
    if !output.status.success() {
        return Err(format!(
            "Provider command '{}' failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn stream(provider: &Provider, item: &ProviderItem) -> Result<Track, String> {
    let url = if let Some(command) = provider.stream_cmd.as_deref() {
        let output = run_command(command, &[("{url}", &item.url)])?;
        output
            .lines()
            .map(str::trim)
            .find(|line| is_http_url(line))
            .ok_or_else(|| "The provider stream command did not return an HTTP URL".to_string())?
            .to_string()
    } else {
        item.url.clone()
    };

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("kanono-x")
        .join("streams");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Could not create stream cache: {}", e))?;
    let path = cache_dir.join(format!("{}.mp3", sanitize_filename(&item.id)));
    let bytes = reqwest::blocking::get(&url)
        .map_err(|e| format!("Provider stream request failed: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Provider stream returned an error: {}", e))?
        .bytes()
        .map_err(|e| format!("Could not read provider stream: {}", e))?;
    fs::write(&path, bytes).map_err(|e| format!("Could not cache provider stream: {}", e))?;
    Ok(item.to_track(Some(path.to_string_lossy().to_string()), Some(url)))
}

pub fn download(
    provider: &Provider,
    item: &ProviderItem,
    output_dir: &Path,
) -> Result<DownloadResult, String> {
    let command = provider
        .download_cmd
        .as_deref()
        .ok_or_else(|| format!("Provider '{}' has no download command", provider.name))?;
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Could not create library folder: {}", e))?;
    let db =
        LibraryDb::open_default().map_err(|e| format!("Could not open library index: {}", e))?;
    let candidate = DuplicateCandidate {
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration: item.duration,
        track_number: item.track_number,
        musicbrainz_recording_id: item.musicbrainz_recording_id.clone(),
        isrc: item.isrc.clone(),
        fingerprint: item.fingerprint.clone(),
    };
    let matches = db
        .find_duplicates(&candidate)
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
    let policy = db.duplicate_policy();
    if let Some(existing) = &existing {
        let existing_path = existing.track.file_path.clone();
        match policy {
            DuplicatePolicy::Skip => {
                return Ok(DownloadResult {
                    track: existing.track.clone(),
                    disposition: DownloadDisposition::Reused,
                    existing_path,
                    reason: Some(existing.reason.label().to_string()),
                });
            }
            DuplicatePolicy::Ask => {
                return Err(format!(
                    "Existing track found at {}; change Duplicate policy to Allow or Replace to continue",
                    existing_path.as_deref().unwrap_or("unknown path")
                ));
            }
            DuplicatePolicy::Allow | DuplicatePolicy::Replace => {}
        }
    }

    let staging_dir = output_dir.join(format!(
        ".kanono-download-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Could not create download staging folder: {}", e))?;
    let output = run_command(
        command,
        &[
            ("{url}", &item.url),
            ("{output_dir}", &staging_dir.to_string_lossy()),
        ],
    );
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };
    let path = match output
        .lines()
        .map(str::trim)
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| newest_audio_file(&staging_dir))
    {
        Some(path) => path,
        None => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(
                "The provider did not produce an audio file in the library folder".to_string(),
            );
        }
    };
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("downloaded-track.mp3");
    let downloaded_track = load_local_track(&path)
        .ok_or_else(|| "The downloaded audio file could not be parsed".to_string())?;
    let mut album = downloaded_track.album.clone();
    if album.eq_ignore_ascii_case("unknown album")
        || album.eq_ignore_ascii_case("external provider")
    {
        album = item.album.clone().unwrap_or_else(|| "Singles".to_string());
    }
    let track_number = downloaded_track.track_number.or(item.track_number);
    let destination = if policy == DuplicatePolicy::Replace {
        existing
            .as_ref()
            .and_then(|matched| matched.track.file_path.as_deref())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                organized_path(
                    output_dir,
                    &downloaded_track.artist,
                    &album,
                    filename,
                    false,
                )
            })
    } else {
        organized_path(output_dir, &downloaded_track.artist, &album, filename, true)
    };
    move_file(&path, &destination)?;
    let _ = fs::remove_dir_all(&staging_dir);
    let mut downloaded_track = load_local_track(&destination)
        .ok_or_else(|| "The downloaded audio file could not be parsed after moving".to_string())?;
    downloaded_track.support_url = item.support_url.clone();
    downloaded_track.album = album;
    downloaded_track.track_number = track_number;
    downloaded_track.file_path = Some(destination.to_string_lossy().to_string());
    Ok(DownloadResult {
        track: downloaded_track,
        disposition: DownloadDisposition::Downloaded,
        existing_path: existing.and_then(|matched| matched.track.file_path),
        reason: None,
    })
}

pub fn download_many(
    provider: &Provider,
    items: &[ProviderItem],
    output_dir: &Path,
) -> Vec<Result<DownloadResult, String>> {
    items
        .iter()
        .map(|item| download(provider, item, output_dir))
        .collect()
}

fn unique_path(dir: &Path, filename: &str) -> PathBuf {
    let initial = dir.join(filename);
    if !initial.exists() {
        return initial;
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("track");
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp3");
    for index in 2.. {
        let candidate = dir.join(format!("{stem} ({index}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn move_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create destination folder: {}", e))?;
    }
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|e| format!("Could not replace existing audio file: {}", e))?;
    }
    fs::rename(source, destination).or_else(|_| {
        fs::copy(source, destination)
            .map(|_| ())
            .map_err(|e| format!("Could not move downloaded audio file: {}", e))?;
        fs::remove_file(source).map_err(|e| format!("Could not clean staged audio file: {}", e))
    })
}

fn run_command(template: &str, replacements: &[(&str, &str)]) -> Result<String, String> {
    let mut command = template.to_string();
    for (placeholder, value) in replacements {
        command = replace_placeholder(&command, placeholder, value);
    }
    // Provider configs commonly use `\t` in yt-dlp's --print format. Convert
    // it after placeholder expansion so shell_words preserves real separators.
    command = command.replace("\\t", "\t");
    let parts = split(&command).map_err(|e| format!("Invalid provider command: {}", e))?;
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| "Provider command is empty".to_string())?;
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Could not start provider command '{}': {}", program, e))?;
    if !output.status.success() {
        return Err(format!(
            "Provider command '{}' failed: {}",
            program,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn replace_placeholder(command: &str, placeholder: &str, value: &str) -> String {
    let mut result = String::with_capacity(command.len() + value.len());
    let mut cursor = 0;
    while let Some(relative) = command[cursor..].find(placeholder) {
        let start = cursor + relative;
        result.push_str(&command[cursor..start]);
        let inside_double_quotes = command[..start]
            .chars()
            .rev()
            .take_while(|character| *character != '\n')
            .filter(|character| *character == '"')
            .count()
            % 2
            == 1;
        if inside_double_quotes {
            result.push_str(&value.replace('\\', "\\\\").replace('"', "\\\""));
        } else {
            result.push_str(&shell_words::quote(value));
        }
        cursor = start + placeholder.len();
    }
    result.push_str(&command[cursor..]);
    result
}

fn newest_audio_file(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(
                        e.to_ascii_lowercase().as_str(),
                        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus"
                    )
                })
        })
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn sanitize_filename(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "stream".to_string()
    } else {
        sanitized
    }
}

fn is_youtube_url(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with("https://youtube.com/")
        || value.starts_with("http://youtube.com/")
        || value.contains("://www.youtube.com/")
        || value.contains("://music.youtube.com/")
        || value.contains("://m.youtube.com/")
        || value.starts_with("https://youtu.be/")
        || value.starts_with("http://youtu.be/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_quoted_query_placeholders() {
        let command = replace_placeholder(
            r#"yt-dlp "ytsearch10:{query}" --print "%(id)s\t%(title)s""#,
            "{query}",
            "lofi hip hop",
        );
        assert!(command.contains(r#""ytsearch10:lofi hip hop""#));
        assert!(!command.contains("ytsearch10:'lofi hip hop'"));
    }

    #[test]
    fn parses_tab_separated_provider_output() {
        let provider = Provider {
            name: "test".to_string(),
            search_cmd: Some(
                "printf 'abc\\tTitle\\tArtist\\t12\\thttps://example.com/a\\tfalse\\thttps://example.com'"
                    .to_string(),
            ),
            stream_cmd: None,
            download_cmd: None,
        };
        let results = search(&provider, "ignored").expect("provider search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "abc");
        assert_eq!(results[0].title, "Title");
        assert_eq!(results[0].duration, 12.0);
    }

    #[test]
    fn recognizes_youtube_and_music_playlist_urls() {
        assert!(is_youtube_url("https://www.youtube.com/playlist?list=abc"));
        assert!(is_youtube_url(
            "https://music.youtube.com/playlist?list=abc"
        ));
        assert!(is_youtube_url("https://youtu.be/video"));
        assert!(!is_youtube_url("https://example.com/playlist"));
    }

    #[test]
    #[ignore = "requires network access to YouTube"]
    fn configured_yt_dlp_provider_returns_results() {
        let provider = load_configured_providers()
            .expect("provider configuration should parse")
            .into_iter()
            .find(|provider| provider.name == "yt-dlp")
            .expect("yt-dlp provider should be configured");
        let results = search(&provider, "lofi hip hop").expect("yt-dlp search should succeed");
        assert!(!results.is_empty());
        assert!(results
            .iter()
            .all(|item| !item.id.is_empty() && !item.url.is_empty()));
    }
}

pub fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
