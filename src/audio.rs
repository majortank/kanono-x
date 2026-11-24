use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use crate::library::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    RepeatOne,
    RepeatAll,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::RepeatAll,
            RepeatMode::RepeatAll => RepeatMode::RepeatOne,
            RepeatMode::RepeatOne => RepeatMode::Off,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RepeatMode::Off => "Repeat: Off",
            RepeatMode::RepeatAll => "Repeat: All 🔁",
            RepeatMode::RepeatOne => "Repeat: Track 🔂",
        }
    }
}

fn open_decoder(path: &str) -> Result<rodio::Decoder<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open audio file: {}", e))?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut builder = rodio::Decoder::builder()
        .with_data(BufReader::new(file))
        .with_seekable(true);
    if len > 0 {
        builder = builder.with_byte_len(len);
    }
    builder.build().map_err(|e| format!("Failed to decode audio: {}", e))
}

pub struct AudioEngine {
    _handle: Option<rodio::MixerDeviceSink>,
    player: Option<rodio::Player>,
    pub current_track: Option<Track>,
    pub is_playing: bool,
    pub is_paused: bool,
    pub volume: f32,
    pub is_muted: bool,
    pub current_pos: f32,
    pub total_duration: f32,
    pub seek_drag_pos: Option<f32>,
    pos_offset: f32,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub error_message: Option<String>,
}

impl AudioEngine {
    pub fn new() -> Self {
        // A larger buffer and a quiet error callback avoid crackles and console error spam
        let handle = rodio::DeviceSinkBuilder::from_default_device()
            .ok()
            .and_then(|b| {
                b.with_buffer_size(rodio::cpal::BufferSize::Fixed(8192))
                    .with_error_callback(|_| {})
                    .open_stream()
                    .ok()
            })
            .or_else(|| rodio::DeviceSinkBuilder::open_default_sink().ok());
        let player = handle.as_ref().map(|h| rodio::Player::connect_new(&h.mixer()));
        let mut engine = Self {
            _handle: handle,
            player,
            current_track: None,
            is_playing: false,
            is_paused: false,
            volume: 0.8,
            is_muted: false,
            current_pos: 0.0,
            total_duration: 0.0,
            seek_drag_pos: None,
            pos_offset: 0.0,
            repeat_mode: RepeatMode::RepeatAll,
            shuffle: false,
            error_message: None,
        };
        engine.set_volume(0.8);
        engine
    }

    pub fn play_track(&mut self, track: Track) -> Result<(), String> {
        let file_path = track.file_path.clone().ok_or_else(|| "Track has no file path".to_string())?;

        let decoder = open_decoder(&file_path)?;
        let mut track = track;
        self.pos_offset = 0.0;
        if track.duration <= 0.0 {
            use rodio::Source;
            if let Some(d) = decoder.total_duration() {
                track.duration = d.as_secs_f32();
            }
        }

        if let Some(player) = &self.player {
            player.stop();
            player.clear();
            player.append(decoder);
            player.set_volume(if self.is_muted { 0.0 } else { self.volume });
            player.play();

            self.current_track = Some(track.clone());
            self.total_duration = track.duration;
            self.current_pos = 0.0;
            self.is_playing = true;
            self.is_paused = false;
            self.error_message = None;
            Ok(())
        } else {
            Err("No audio output device available".to_string())
        }
    }

    pub fn toggle_play_pause(&mut self) {
        if let Some(player) = &self.player {
            if self.is_playing {
                if self.is_paused {
                    player.play();
                    self.is_paused = false;
                } else {
                    player.pause();
                    self.is_paused = true;
                }
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(player) = &self.player {
            player.stop();
            player.clear();
        }
        self.is_playing = false;
        self.is_paused = false;
        self.current_pos = 0.0;
    }

    pub fn seek_to(&mut self, target_secs: f32) {
        if !target_secs.is_finite() {
            return;
        }
        let max = if self.total_duration.is_finite() { self.total_duration.max(0.0) } else { 0.0 };
        let target_secs = if max > 0.0 { target_secs.clamp(0.0, max) } else { target_secs.max(0.0) };
        self.current_pos = target_secs;

        if let Some(player) = &self.player {
            let dur = Duration::from_secs_f32(target_secs);
            if player.try_seek(dur).is_ok() {
                self.pos_offset = 0.0;
            } else if let Some(path) = self.current_track.as_ref().and_then(|t| t.file_path.clone()) {
                // Fallback: reload the file and skip ahead to the target
                if let Ok(decoder) = open_decoder(&path) {
                    use rodio::Source;
                    player.stop();
                    player.clear();
                    player.append(decoder.skip_duration(dur));
                    self.pos_offset = target_secs;
                    if !self.is_paused {
                        player.play();
                    }
                }
            }
        }
    }

    pub fn seek_relative(&mut self, delta_secs: f32) {
        let target = self.current_pos + delta_secs;
        self.seek_to(target);
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(player) = &self.player {
            player.set_volume(if self.is_muted { 0.0 } else { self.volume });
        }
    }

    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
        self.set_volume(self.volume);
    }

    pub fn update(&mut self) -> bool {
        // Returns true if current track has finished
        if let Some(player) = &self.player {
            if self.is_playing && !self.is_paused {
                // If user is not dragging seek slider, update position
                if self.seek_drag_pos.is_none() {
                    let pos = player.get_pos();
                    self.current_pos = self.pos_offset + pos.as_secs_f32();
                }

                // Check if track ended
                if player.empty() {
                    return true;
                }
            }
        }
        false
    }
}
