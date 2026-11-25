use crate::library::{load_local_track, Track};
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
            album: "External Provider".to_string(),
            duration: self.duration,
            file_path,
            is_stream: stream_url.is_some(),
            stream_url,
            support_url: self.support_url.clone(),
            track_number: None,
            bitrate: None,
            sample_rate: None,
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
                search_cmd: Some("yt-dlp \"ytsearch10:{query}\" --flat-playlist --print \"%(id)s\\t%(title)s\\t%(uploader)s\\t0\\t%(webpage_url)s\\tfalse\\t%(uploader_url)s\"".to_string()),
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
    let command = provider
        .search_cmd
        .as_deref()
        .ok_or_else(|| format!("Provider '{}' has no search command", provider.name))?;
    let output = run_command(command, &[("{query}", query)])?;
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
        });
    }
    Ok(items)
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
) -> Result<Track, String> {
    let command = provider
        .download_cmd
        .as_deref()
        .ok_or_else(|| format!("Provider '{}' has no download command", provider.name))?;
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Could not create library folder: {}", e))?;
    let before = newest_audio_file(output_dir);
    let output = run_command(
        command,
        &[
            ("{url}", &item.url),
            ("{output_dir}", &output_dir.to_string_lossy()),
        ],
    )?;
    let path = output
        .lines()
        .map(str::trim)
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| newest_audio_file(output_dir).filter(|p| Some(p) != before.as_ref()))
        .ok_or_else(|| {
            "The provider did not produce an audio file in the library folder".to_string()
        })?;
    load_local_track(&path)
        .map(|mut track| {
            track.support_url = item.support_url.clone();
            track
        })
        .ok_or_else(|| "The downloaded audio file could not be parsed".to_string())
}

fn run_command(template: &str, replacements: &[(&str, &str)]) -> Result<String, String> {
    let mut command = template.to_string();
    for (placeholder, value) in replacements {
        command = command.replace(placeholder, &shell_words::quote(value));
    }
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

pub fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
