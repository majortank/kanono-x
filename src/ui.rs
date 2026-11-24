use eframe::egui::{self, Color32, RichText, Stroke};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::audio::{AudioEngine, RepeatMode};
use crate::audius::{self, AudiusItem};
use crate::folder_tree::{FolderHierarchy, FolderTreeNode};
use crate::library::{format_time, scan_directory, Track};
use crate::library_db::{DbStats, LibraryDb};
use crate::providers::{self, Provider, ProviderItem};
use crate::theme::{PlayerTheme, ThemePalette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewTab {
    NowPlaying,
    Library,
    Discover,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterField {
    All,
    Title,
    Artist,
    Album,
    FileName,
    Folder,
}

impl FilterField {
    fn label(&self) -> &'static str {
        match self {
            FilterField::All => "All Fields",
            FilterField::Title => "Title",
            FilterField::Artist => "Artist",
            FilterField::Album => "Album",
            FilterField::FileName => "File name",
            FilterField::Folder => "Folder",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Added,
    Title,
    Artist,
    Album,
    Duration,
    Folder,
}

impl SortKey {
    fn label(&self) -> &'static str {
        match self {
            SortKey::Added => "Order added",
            SortKey::Title => "Title",
            SortKey::Artist => "Artist",
            SortKey::Album => "Album",
            SortKey::Duration => "Duration",
            SortKey::Folder => "Folder",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarMode {
    FolderTree,
    Albums,
    Artists,
}

pub enum BgTaskMsg {
    SearchComplete(Result<DiscoverResults, String>),
    DownloadComplete(Result<Track, String>),
    StreamComplete(Result<Track, String>),
    Scanned(Vec<Track>, String),
}

pub enum DiscoverResults {
    Audius(Vec<AudiusItem>),
    Provider(Vec<ProviderItem>),
}

pub struct KanonoApp {
    pub audio: AudioEngine,
    pub playlist: Vec<Track>,
    pub current_index: Option<usize>,
    pub current_tab: ViewTab,

    // Themes & CRT Effects
    pub current_theme: PlayerTheme,
    pub crt_scanlines: bool,
    pub last_theme_applied: Option<PlayerTheme>,

    // SQLite Database
    pub db: LibraryDb,
    pub db_stats: DbStats,

    // Foobar Folder Traversal & Filtering
    pub folder_tree: FolderHierarchy,
    pub sidebar_mode: SidebarMode,
    pub library_filter: String,
    pub filter_field: FilterField,
    pub album_filter: Option<String>,
    pub artist_filter: Option<String>,
    pub sort_key: SortKey,
    pub sort_desc: bool,
    pub queue: Vec<Track>,

    pub scanning: usize,
    pub saved: crate::library::SavedLibrary,
    pub provider_search_input: String,
    pub providers: Vec<Provider>,
    pub active_provider: Option<usize>,
    pub audius_results: Vec<AudiusItem>,
    pub provider_results: Vec<ProviderItem>,
    pub discover_is_searching: bool,
    pub discover_is_downloading: bool,
    pub discover_status_message: String,
    pub downloaded_history: Vec<Track>,
    pub provider_config_error: Option<String>,

    // Channels for background tasks
    pub bg_sender: Sender<BgTaskMsg>,
    pub bg_receiver: Receiver<BgTaskMsg>,

    pub library_dir: PathBuf,

    // Visualizer state
    pub visualizer_phase: f32,

    // Screenshot helper
    pub screenshot_path: Option<String>,
    pub frame_count: u32,
    pub screenshot_sent: bool,

    // Notification toast
    pub toast_message: Option<(String, f32)>,
}

impl KanonoApp {
    pub fn new(screenshot_path: Option<String>) -> Self {
        let library_dir = crate::library::ensure_library_dir();
        let (bg_sender, bg_receiver) = channel();
        let (providers, provider_config_error) = match providers::load_configured_providers() {
            Ok(providers) => (providers, None),
            Err(error) => (Vec::new(), Some(error)),
        };

        // Open SQLite Database
        let mut db = LibraryDb::open_default().unwrap_or_else(|_| {
            LibraryDb::open(&crate::library::data_dir().join("kanono_x_library.db"))
                .expect("Failed to open SQLite database")
        });

        // Load saved theme from SQLite if available
        let saved_theme = db
            .get_setting("theme")
            .and_then(|s| match s.as_str() {
                "CrtGreen" => Some(PlayerTheme::CrtGreen),
                "CrtAmber" => Some(PlayerTheme::CrtAmber),
                "CrtCyan" => Some(PlayerTheme::CrtCyan),
                "OmarchyEmerald" => Some(PlayerTheme::OmarchyEmerald),
                "OmarchyCyberpunk" => Some(PlayerTheme::OmarchyCyberpunk),
                "OmarchyNord" => Some(PlayerTheme::OmarchyNord),
                "OmarchyDracula" => Some(PlayerTheme::OmarchyDracula),
                "SolarizedLight" => Some(PlayerTheme::SolarizedLight),
                _ => None,
            })
            .unwrap_or(PlayerTheme::CrtGreen);

        let crt_scanlines = db
            .get_setting("crt_scanlines")
            .map(|s| s == "true")
            .unwrap_or(true);

        // Pre-load saved folders into SQLite
        let saved_folders = crate::library::load_saved();
        for f in &saved_folders.folders {
            let _ = db.save_folder(f);
        }

        let db_stats = db.get_stats();

        // Load cached tracks from SQLite
        let initial_tracks = db.load_all_tracks().unwrap_or_default();

        let mut app = Self {
            audio: AudioEngine::new(),
            playlist: initial_tracks.clone(),
            current_index: None,
            current_tab: ViewTab::NowPlaying,

            current_theme: saved_theme,
            crt_scanlines,
            last_theme_applied: None,

            db,
            db_stats,

            folder_tree: FolderHierarchy::new(),
            sidebar_mode: SidebarMode::FolderTree,
            library_filter: String::new(),
            filter_field: FilterField::All,
            album_filter: None,
            artist_filter: None,
            sort_key: SortKey::Added,
            sort_desc: false,
            queue: Vec::new(),

            scanning: 0,
            saved: saved_folders,
            provider_search_input: String::new(),
            providers,
            active_provider: None,
            audius_results: Vec::new(),
            provider_results: Vec::new(),
            discover_is_searching: false,
            discover_is_downloading: false,
            discover_status_message: "Ready to search Audius or a configured provider.".to_string(),
            downloaded_history: Vec::new(),
            provider_config_error,

            bg_sender,
            bg_receiver,

            library_dir,

            visualizer_phase: 0.0,
            screenshot_path,
            frame_count: 0,
            screenshot_sent: false,
            toast_message: None,
        };

        app.rebuild_folder_tree();
        app.load_initial_tracks();
        app
    }

    pub fn rebuild_folder_tree(&mut self) {
        let paths: Vec<&str> = self
            .playlist
            .iter()
            .filter_map(|t| t.file_path.as_deref())
            .collect();
        self.folder_tree.rebuild_from_paths(paths);
    }

    pub fn load_initial_tracks(&mut self) {
        let mut dirs = vec![self.library_dir.clone()];
        dirs.extend(self.saved.folders.iter().map(PathBuf::from));
        let files: Vec<PathBuf> = self.saved.files.iter().map(PathBuf::from).collect();
        self.start_scan(dirs, files, "Library refreshed".to_string());
    }

    fn start_scan(&mut self, dirs: Vec<PathBuf>, files: Vec<PathBuf>, label: String) {
        self.scanning += 1;
        let sender = self.bg_sender.clone();
        thread::spawn(move || {
            let mut tracks = Vec::new();
            for d in dirs {
                tracks.extend(scan_directory(&d));
            }
            for f in files {
                if let Some(t) = crate::library::load_local_track(&f) {
                    tracks.push(t);
                }
            }
            crate::library::flush_cache();
            let _ = sender.send(BgTaskMsg::Scanned(tracks, label));
        });
    }

    fn finish_scan(&mut self, tracks: Vec<Track>, label: String) {
        self.scanning = self.scanning.saturating_sub(1);
        let found = tracks.len();

        // Upsert to SQLite WAL database
        let _ = self.db.upsert_tracks_batch(&tracks);
        self.db_stats = self.db.get_stats();

        let added = self.add_tracks(tracks);
        self.rebuild_folder_tree();
        self.show_toast(format!("{} ({} found, {} new)", label, found, added));

        if self.current_index.is_none() && !self.playlist.is_empty() {
            self.current_index = Some(0);
            let t = self.playlist[0].clone();
            self.audio.total_duration = t.duration;
            self.audio.current_track = Some(t);
        }
    }

    pub fn add_tracks(&mut self, tracks: Vec<Track>) -> usize {
        let mut added = 0;
        for t in tracks {
            if !self.playlist.iter().any(|p| p.id == t.id) {
                self.playlist.push(t);
                added += 1;
            }
        }
        added
    }

    pub fn rescan_library(&mut self) {
        let cur_id = self
            .current_index
            .and_then(|i| self.playlist.get(i))
            .map(|t| t.id.clone());
        self.playlist.retain(|t| {
            t.is_stream
                || t.file_path
                    .as_ref()
                    .map(|p| Path::new(p).exists())
                    .unwrap_or(false)
        });
        self.current_index = cur_id.and_then(|id| self.playlist.iter().position(|t| t.id == id));
        self.load_initial_tracks();
    }

    pub fn enqueue(&mut self, index: usize, front: bool) {
        if let Some(t) = self.playlist.get(index).cloned() {
            let name = t.title.clone();
            if front {
                self.queue.insert(0, t);
                self.show_toast(format!("Playing next: {}", name));
            } else {
                self.queue.push(t);
                self.show_toast(format!("Added to queue: {}", name));
            }
        }
    }

    pub fn remove_track(&mut self, idx: usize) {
        if idx >= self.playlist.len() {
            return;
        }
        let id = self.playlist[idx].id.clone();
        let _ = self.db.delete_track(&id);
        self.db_stats = self.db.get_stats();

        self.playlist.remove(idx);
        self.queue.retain(|t| t.id != id);
        self.rebuild_folder_tree();

        match self.current_index {
            Some(c) if c == idx => {
                self.audio.stop();
                self.current_index = None;
            }
            Some(c) if c > idx => self.current_index = Some(c - 1),
            _ => {}
        }
    }

    pub fn add_folder(&mut self, folder: PathBuf) {
        let f = folder.to_string_lossy().to_string();
        if !self.saved.folders.contains(&f) {
            self.saved.folders.push(f.clone());
            crate::library::save_saved(&self.saved);
        }
        let _ = self.db.save_folder(&f);
        self.db_stats = self.db.get_stats();
        self.start_scan(vec![folder], Vec::new(), "Folder scanned".to_string());
    }

    pub fn remove_folder_from_library(&mut self, folder: &str) {
        self.saved.folders.retain(|f| f != folder);
        crate::library::save_saved(&self.saved);
        let _ = self.db.remove_folder(folder);

        // Remove tracks originating in this folder
        let folder_path = Path::new(folder);
        self.playlist.retain(|t| {
            if let Some(p) = &t.file_path {
                !Path::new(p).starts_with(folder_path)
            } else {
                true
            }
        });

        self.rebuild_folder_tree();
        self.db_stats = self.db.get_stats();
        self.show_toast(format!("Removed folder: {}", folder));
    }

    pub fn play_index(&mut self, index: usize) {
        if index < self.playlist.len() {
            self.current_index = Some(index);
            let track = self.playlist[index].clone();
            let title = track.title.clone();
            let track_id = track.id.clone();

            match self.audio.play_track(track) {
                Ok(_) => {
                    let _ = self.db.record_play(&track_id);
                    if let Some(t) = self.audio.current_track.clone() {
                        self.playlist[index] = t;
                    }
                    self.show_toast(format!("Playing: {}", title));
                }
                Err(e) => {
                    self.audio.stop();
                    self.show_toast(format!("Playback error: {}", e));
                }
            }
        }
    }

    pub fn play_next(&mut self) {
        if self.playlist.is_empty() {
            self.audio.stop();
            return;
        }

        if !self.queue.is_empty() {
            let t = self.queue.remove(0);
            if let Some(i) = self.playlist.iter().position(|p| p.id == t.id) {
                self.play_index(i);
                return;
            }
        }

        if self.audio.shuffle {
            let next_idx = (self.frame_count as usize * 17 + 3) % self.playlist.len();
            self.play_index(next_idx);
            return;
        }

        if let Some(curr) = self.current_index {
            let next = curr + 1;
            if next < self.playlist.len() {
                self.play_index(next);
            } else if self.audio.repeat_mode == RepeatMode::RepeatAll {
                self.play_index(0);
            } else {
                self.audio.stop();
            }
        } else {
            self.play_index(0);
        }
    }

    pub fn play_previous(&mut self) {
        if self.playlist.is_empty() {
            return;
        }

        if self.audio.current_pos > 3.0 {
            self.audio.seek_to(0.0);
            return;
        }

        if let Some(curr) = self.current_index {
            if curr > 0 {
                self.play_index(curr - 1);
            } else if self.audio.repeat_mode == RepeatMode::RepeatAll {
                self.play_index(self.playlist.len().saturating_sub(1));
            } else {
                self.audio.seek_to(0.0);
            }
        } else {
            self.play_index(0);
        }
    }

    pub fn show_toast(&mut self, message: String) {
        self.toast_message = Some((message, 4.0));
    }

    pub fn set_theme(&mut self, theme: PlayerTheme) {
        self.current_theme = theme;
        let _ = self.db.set_setting(
            "theme",
            match theme {
                PlayerTheme::CrtGreen => "CrtGreen",
                PlayerTheme::CrtAmber => "CrtAmber",
                PlayerTheme::CrtCyan => "CrtCyan",
                PlayerTheme::OmarchyEmerald => "OmarchyEmerald",
                PlayerTheme::OmarchyCyberpunk => "OmarchyCyberpunk",
                PlayerTheme::OmarchyNord => "OmarchyNord",
                PlayerTheme::OmarchyDracula => "OmarchyDracula",
                PlayerTheme::SolarizedLight => "SolarizedLight",
            },
        );
    }

    pub fn start_discover_search(&mut self) {
        let query = self.provider_search_input.trim().to_string();
        if query.is_empty() {
            return;
        }

        self.discover_is_searching = true;
        self.discover_status_message = format!("Searching for '{}'...", query);

        let sender = self.bg_sender.clone();
        let provider = self
            .active_provider
            .and_then(|index| self.providers.get(index).cloned());
        thread::spawn(move || {
            let res = provider
                .map(|provider| providers::search(&provider, &query).map(DiscoverResults::Provider))
                .unwrap_or_else(|| audius::search(&query).map(DiscoverResults::Audius));
            let _ = sender.send(BgTaskMsg::SearchComplete(res));
        });
    }

    pub fn start_audius_download(&mut self, item: AudiusItem) {
        if !item.is_downloadable {
            return;
        }
        self.discover_is_downloading = true;
        self.discover_status_message =
            format!("Downloading '{}' to your music library...", item.title);
        self.show_toast(format!("Downloading: {}", item.title));

        let sender = self.bg_sender.clone();
        let library_dir = self.library_dir.clone();

        thread::spawn(move || {
            let res = audius::download_to_library(&item, &library_dir);
            let _ = sender.send(BgTaskMsg::DownloadComplete(res));
        });
    }

    pub fn start_audius_stream(&mut self, item: AudiusItem) {
        self.discover_status_message = format!("Connecting to Audius for '{}'...", item.title);
        self.show_toast(format!("Connecting stream: {}", item.title));

        let sender = self.bg_sender.clone();

        thread::spawn(move || {
            let res = audius::stream_to_cache(&item);
            let _ = sender.send(BgTaskMsg::StreamComplete(res));
        });
    }

    pub fn start_provider_stream(&mut self, provider: Provider, item: ProviderItem) {
        self.discover_status_message =
            format!("Connecting to {} for '{}'...", provider.name, item.title);
        self.show_toast(format!("Connecting stream: {}", item.title));
        let sender = self.bg_sender.clone();
        thread::spawn(move || {
            let res = providers::stream(&provider, &item);
            let _ = sender.send(BgTaskMsg::StreamComplete(res));
        });
    }

    pub fn start_provider_download(&mut self, provider: Provider, item: ProviderItem) {
        if !item.downloadable {
            return;
        }
        self.discover_is_downloading = true;
        self.discover_status_message =
            format!("Downloading '{}' to your music library...", item.title);
        self.show_toast(format!("Downloading: {}", item.title));
        let sender = self.bg_sender.clone();
        let library_dir = self.library_dir.clone();
        thread::spawn(move || {
            let res = providers::download(&provider, &item, &library_dir);
            let _ = sender.send(BgTaskMsg::DownloadComplete(res));
        });
    }

    pub fn process_bg_messages(&mut self) {
        while let Ok(msg) = self.bg_receiver.try_recv() {
            match msg {
                BgTaskMsg::Scanned(tracks, label) => self.finish_scan(tracks, label),
                BgTaskMsg::SearchComplete(res) => {
                    self.discover_is_searching = false;
                    match res {
                        Ok(DiscoverResults::Audius(items)) => {
                            self.audius_results = items;
                            self.provider_results.clear();
                            self.discover_status_message =
                                format!("Found {} Audius result(s).", self.audius_results.len());
                        }
                        Ok(DiscoverResults::Provider(items)) => {
                            self.provider_results = items;
                            self.audius_results.clear();
                            self.discover_status_message = format!(
                                "Found {} provider result(s).",
                                self.provider_results.len()
                            );
                        }
                        Err(e) => {
                            self.discover_status_message = format!("Search failed: {}", e);
                            self.show_toast(format!("Search error: {}", e));
                        }
                    }
                }
                BgTaskMsg::DownloadComplete(res) => {
                    self.discover_is_downloading = false;
                    match res {
                        Ok(track) => {
                            let title = track.title.clone();
                            self.discover_status_message =
                                format!("Downloaded '{}' and added it to the library.", title);
                            let _ = self.db.upsert_track(&track);
                            self.db_stats = self.db.get_stats();
                            self.downloaded_history.push(track.clone());
                            self.playlist.push(track);
                            self.rebuild_folder_tree();
                            self.show_toast(format!("Added to Library: {}", title));
                        }
                        Err(e) => {
                            self.discover_status_message = format!("Download error: {}", e);
                            self.show_toast(format!("Download failed: {}", e));
                        }
                    }
                }
                BgTaskMsg::StreamComplete(res) => match res {
                    Ok(track) => {
                        self.discover_status_message =
                            format!("Streaming online: '{}'.", track.title);
                        self.playlist.push(track);
                        let new_idx = self.playlist.len() - 1;
                        self.play_index(new_idx);
                    }
                    Err(e) => {
                        self.discover_status_message = format!("Stream error: {}", e);
                        self.show_toast(format!("Stream failed: {}", e));
                    }
                },
            }
        }
    }
}

impl eframe::App for KanonoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;
        self.process_bg_messages();

        // Apply theme styling dynamically when changed
        if self.last_theme_applied != Some(self.current_theme) {
            self.current_theme.apply_to_egui(ctx);
            self.last_theme_applied = Some(self.current_theme);
        }

        // PERFORMANCE FIX: Rate-limited repaint to prevent 100% CPU busy-loop spinning
        if self.audio.is_playing && !self.audio.is_paused {
            self.visualizer_phase += 0.06;
            // Smooth 60 FPS repaint during active audio playback (16ms)
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else if self.scanning > 0 || self.discover_is_searching || self.discover_is_downloading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        let dt = ctx.input(|i| i.unstable_dt);
        if let Some((_, left)) = self.toast_message.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.toast_message = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // Handle track end
        let track_finished = self.audio.update();
        if track_finished {
            if self.audio.repeat_mode == RepeatMode::RepeatOne {
                self.audio.seek_to(0.0);
            } else {
                self.play_next();
            }
        }

        // Handle screenshot mode
        if let Some(ref path) = self.screenshot_path {
            ctx.request_repaint();

            ctx.input(|i| {
                for event in &i.raw.events {
                    if let egui::Event::Screenshot { image, .. } = event {
                        let width = image.width() as u32;
                        let height = image.height() as u32;
                        let mut rgba_bytes = Vec::with_capacity(image.pixels.len() * 4);
                        for p in &image.pixels {
                            rgba_bytes.push(p.r());
                            rgba_bytes.push(p.g());
                            rgba_bytes.push(p.b());
                            rgba_bytes.push(p.a());
                        }
                        let _ = image::save_buffer(
                            path,
                            &rgba_bytes,
                            width,
                            height,
                            image::ExtendedColorType::Rgba8,
                        );
                        std::process::exit(0);
                    }
                }
            });

            if self.frame_count >= 120 && !self.screenshot_sent {
                self.screenshot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }

        let palette = self.current_theme.palette();

        // Top Header
        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::new()
                    .fill(palette.bg_panel)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Logo and Branding
                    ui.label(
                        RichText::new("⚡ KANONO X")
                            .color(palette.accent)
                            .size(20.0)
                            .strong(),
                    );

                    // Version Badge
                    let badge_text = if palette.is_crt {
                        "[ADVANCED v2.0 // FOOBAR CRT]"
                    } else {
                        "[ADVANCED v2.0 // FOOBAR EDITION]"
                    };

                    ui.label(
                        RichText::new(badge_text)
                            .size(11.0)
                            .color(palette.badge_text)
                            .monospace(),
                    );

                    ui.add_space(8.0);

                    // Quick Theme Cycler Button
                    if ui
                        .button(
                            RichText::new(format!("🎨 {}", self.current_theme.label())).size(12.0),
                        )
                        .on_hover_text("Click to cycle themes (Omarchy + CRT options)")
                        .clicked()
                    {
                        let next = self.current_theme.next();
                        self.set_theme(next);
                    }

                    // Navigation Tabs
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .selectable_label(
                                self.current_tab == ViewTab::Settings,
                                "⚙ Settings & DB",
                            )
                            .clicked()
                        {
                            self.current_tab = ViewTab::Settings;
                        }
                        if ui
                            .selectable_label(
                                self.current_tab == ViewTab::Discover,
                                "🔎 Discover / Providers",
                            )
                            .clicked()
                        {
                            self.current_tab = ViewTab::Discover;
                        }
                        if ui
                            .selectable_label(
                                self.current_tab == ViewTab::Library,
                                "📚 Library & Folders",
                            )
                            .clicked()
                        {
                            self.current_tab = ViewTab::Library;
                        }
                        if ui
                            .selectable_label(
                                self.current_tab == ViewTab::NowPlaying,
                                "📻 Now Playing",
                            )
                            .clicked()
                        {
                            self.current_tab = ViewTab::NowPlaying;
                        }
                    });
                });
            });

        // Bottom Persistent Player Bar
        egui::TopBottomPanel::bottom("bottom_player_bar")
            .min_height(82.0)
            .frame(
                egui::Frame::new()
                    .fill(palette.bg_panel)
                    .stroke(Stroke::new(1.0_f32, palette.border_subtle))
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ctx, |ui| {
                self.render_bottom_player_bar(ui, &palette);
            });

        // Main Content Area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(palette.bg_root)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                // Toast message banner
                if let Some((msg, _)) = &self.toast_message {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("ℹ ").color(palette.accent));
                            ui.label(RichText::new(msg).strong().color(palette.text_primary));
                        });
                    });
                    ui.add_space(6.0);
                }

                match self.current_tab {
                    ViewTab::NowPlaying => self.render_now_playing_view(ui, &palette),
                    ViewTab::Library => self.render_library_view(ui, &palette),
                    ViewTab::Discover => self.render_discover_view(ui, &palette),
                    ViewTab::Settings => self.render_settings_view(ui, &palette),
                }

                // CRT Scanline Shader Overlay
                if self.crt_scanlines && palette.is_crt {
                    let painter = ui.painter();
                    let clip_rect = ui.clip_rect();
                    let line_stroke =
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 40));
                    let mut y = clip_rect.min.y;
                    while y < clip_rect.max.y {
                        painter.line_segment(
                            [
                                egui::pos2(clip_rect.min.x, y),
                                egui::pos2(clip_rect.max.x, y),
                            ],
                            line_stroke,
                        );
                        y += 4.0;
                    }
                }
            });
    }
}

impl KanonoApp {
    fn render_bottom_player_bar(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.horizontal(|ui| {
            // Left: Track Info
            ui.allocate_ui_with_layout(
                egui::vec2(260.0, 72.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if let Some(track) = &self.audio.current_track {
                        ui.horizontal(|ui| {
                            let title = shorten_end(&track.title, 24);
                            ui.label(
                                RichText::new(title)
                                    .strong()
                                    .size(15.0)
                                    .color(p.text_primary),
                            );
                            if track.is_stream {
                                ui.label(RichText::new("[Stream]").size(10.0).color(p.badge_text));
                            }
                        });
                        ui.label(
                            RichText::new(&track.artist)
                                .size(12.0)
                                .color(p.text_secondary),
                        );
                        let status_str = if self.audio.is_playing {
                            if self.audio.is_paused {
                                "⏸ Paused"
                            } else {
                                "▶ Playing"
                            }
                        } else {
                            "⏹ Stopped"
                        };
                        ui.label(RichText::new(status_str).size(11.0).color(p.accent));
                    } else {
                        ui.label(
                            RichText::new("No track loaded")
                                .italics()
                                .size(14.0)
                                .color(p.text_muted),
                        );
                        ui.label(
                            RichText::new("Select a track from Library or Discover")
                                .size(11.0)
                                .color(p.text_muted),
                        );
                    }
                },
            );

            ui.separator();

            let right_width = 160.0;
            let center_width = (ui.available_width() - right_width - 20.0).max(300.0);

            // Center: Playback Controls and Seek Bar
            ui.allocate_ui_with_layout(
                egui::vec2(center_width, 72.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    // Controls Row
                    ui.horizontal(|ui| {
                        let shuffle_text = if self.audio.shuffle {
                            "🔀 On"
                        } else {
                            "🔀 Off"
                        };
                        if ui
                            .selectable_label(self.audio.shuffle, shuffle_text)
                            .clicked()
                        {
                            self.audio.shuffle = !self.audio.shuffle;
                        }

                        if ui.button("⏮").on_hover_text("Previous Track").clicked() {
                            self.play_previous();
                        }

                        if ui
                            .button("⏪ -10s")
                            .on_hover_text("Jump backward 10 seconds")
                            .clicked()
                        {
                            self.audio.seek_relative(-10.0);
                        }

                        let play_pause_icon = if self.audio.is_playing && !self.audio.is_paused {
                            "⏸ Pause"
                        } else {
                            "▶ Play"
                        };
                        if ui
                            .add_sized(
                                [100.0, 28.0],
                                egui::Button::new(
                                    RichText::new(play_pause_icon).strong().size(14.0),
                                ),
                            )
                            .clicked()
                        {
                            if self.audio.is_playing {
                                self.audio.toggle_play_pause();
                            } else if let Some(idx) = self.current_index {
                                self.play_index(idx);
                            } else if !self.playlist.is_empty() {
                                self.play_index(0);
                            }
                        }

                        if ui
                            .button("+10s ⏩")
                            .on_hover_text("Jump forward 10 seconds")
                            .clicked()
                        {
                            self.audio.seek_relative(10.0);
                        }

                        if ui.button("⏭").on_hover_text("Next Track").clicked() {
                            self.play_next();
                        }

                        if ui.button("⏹").on_hover_text("Stop playback").clicked() {
                            self.audio.stop();
                        }

                        if ui.button(self.audio.repeat_mode.label()).clicked() {
                            self.audio.repeat_mode = self.audio.repeat_mode.next();
                        }
                    });

                    ui.add_space(4.0);

                    // Seek Progress Bar
                    ui.horizontal(|ui| {
                        let cur_secs = self.audio.seek_drag_pos.unwrap_or(self.audio.current_pos);
                        let total_secs = self.audio.total_duration.max(0.1);

                        ui.label(
                            RichText::new(format_time(cur_secs))
                                .size(12.0)
                                .monospace()
                                .color(p.text_primary),
                        );

                        let mut slider_val = cur_secs;
                        let slider = egui::Slider::new(&mut slider_val, 0.0..=total_secs)
                            .show_value(false)
                            .trailing_fill(true);

                        let seek_width = (ui.available_width() - 65.0).max(100.0);
                        let response = ui.add_sized([seek_width, 18.0], slider);

                        if response.changed() || response.dragged() {
                            self.audio.seek_drag_pos = Some(slider_val);
                        }

                        if response.drag_stopped() || (response.changed() && !response.dragged()) {
                            if self.audio.is_playing {
                                self.audio.seek_to(slider_val);
                            }
                            self.audio.seek_drag_pos = None;
                        }

                        ui.label(
                            RichText::new(format_time(total_secs))
                                .size(12.0)
                                .monospace()
                                .color(p.text_secondary),
                        );
                    });
                },
            );

            ui.separator();

            // Right: Volume & Mute Control
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, 72.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let mute_icon = if self.audio.is_muted || self.audio.volume == 0.0 {
                            "🔇"
                        } else if self.audio.volume < 0.5 {
                            "🔉"
                        } else {
                            "🔊"
                        };
                        if ui.button(mute_icon).on_hover_text("Toggle Mute").clicked() {
                            self.audio.toggle_mute();
                        }

                        let mut vol = if self.audio.is_muted {
                            0.0
                        } else {
                            self.audio.volume
                        };
                        if ui
                            .add(egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false))
                            .changed()
                        {
                            if self.audio.is_muted {
                                self.audio.is_muted = false;
                            }
                            self.audio.set_volume(vol);
                        }
                    });
                    let vol_pct = (if self.audio.is_muted {
                        0.0
                    } else {
                        self.audio.volume
                    } * 100.0)
                        .round() as u32;
                    ui.label(
                        RichText::new(format!("Volume: {}%", vol_pct))
                            .size(11.0)
                            .color(p.text_secondary),
                    );
                },
            );
        });
    }

    fn render_now_playing_view(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.add_space(8.0);

        ui.columns(2, |columns| {
            // Left Column: Artwork and Animated Visualizer
            columns[0].vertical(|ui| {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);

                        // CRT / Vinyl Turntable Display Card
                        let (rect, _response) =
                            ui.allocate_exact_size(egui::vec2(280.0, 160.0), egui::Sense::hover());
                        let painter = ui.painter();

                        // Background card
                        painter.rect_filled(rect, 8.0, p.bg_card);
                        painter.rect_stroke(
                            rect,
                            8.0,
                            Stroke::new(1.0_f32, p.border),
                            egui::StrokeKind::Inside,
                        );

                        // Disc graphic
                        let center = rect.center();
                        let radius = 54.0;
                        painter.circle_filled(center, radius, Color32::from_rgb(14, 16, 24));
                        painter.circle_stroke(center, radius, Stroke::new(2.0_f32, p.accent));
                        painter.circle_stroke(
                            center,
                            radius * 0.70,
                            Stroke::new(1.0_f32, p.border_subtle),
                        );
                        painter.circle_filled(center, 14.0, p.accent_hover);

                        ui.add_space(14.0);

                        // Animated Frequency Spectrum Bars with Palette Colors
                        let (vis_rect, _) =
                            ui.allocate_exact_size(egui::vec2(320.0, 52.0), egui::Sense::hover());
                        let vis_painter = ui.painter();
                        let num_bars = 32;
                        let bar_width = 7.0;
                        let spacing = 3.0;
                        let start_x = vis_rect.left()
                            + (vis_rect.width() - (num_bars as f32 * (bar_width + spacing))) / 2.0;

                        for i in 0..num_bars {
                            let factor = if self.audio.is_playing && !self.audio.is_paused {
                                let f = (self.visualizer_phase + i as f32 * 0.32).sin().abs();
                                let f2 =
                                    (self.visualizer_phase * 1.35 + i as f32 * 0.20).cos().abs();
                                (f * 0.65 + f2 * 0.35).clamp(0.12, 1.0)
                            } else {
                                0.08
                            };

                            let bar_height = 6.0 + factor * 42.0;
                            let bar_x = start_x + i as f32 * (bar_width + spacing);
                            let bar_y = vis_rect.bottom() - bar_height;

                            let color = if i < 11 {
                                p.visualizer_low
                            } else if i < 22 {
                                p.visualizer_mid
                            } else {
                                p.visualizer_high
                            };

                            vis_painter.rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(bar_x, bar_y),
                                    egui::vec2(bar_width, bar_height),
                                ),
                                2.0,
                                color,
                            );
                        }
                        ui.add_space(6.0);
                    });
                });

                ui.add_space(8.0);

                // Quick Seeking & Jump Panel
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("⚡ Quick Jump:").strong().color(p.accent));
                        if ui.button("-30s").clicked() {
                            self.audio.seek_relative(-30.0);
                        }
                        if ui.button("-10s").clicked() {
                            self.audio.seek_relative(-10.0);
                        }
                        if ui.button("-5s").clicked() {
                            self.audio.seek_relative(-5.0);
                        }
                        if ui.button("+5s").clicked() {
                            self.audio.seek_relative(5.0);
                        }
                        if ui.button("+10s").clicked() {
                            self.audio.seek_relative(10.0);
                        }
                        if ui.button("+30s").clicked() {
                            self.audio.seek_relative(30.0);
                        }
                        if ui.button("Restart (0:00)").clicked() {
                            self.audio.seek_to(0.0);
                        }
                    });
                });
            });

            // Right Column: Track Details & Active Playlist
            columns[1].vertical(|ui| {
                let mut support_url_to_open = None;
                if let Some(track) = &self.audio.current_track {
                    ui.group(|ui| {
                        ui.heading(RichText::new(&track.title).size(18.0).color(p.text_primary));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Artist: {}", track.artist))
                                .size(14.0)
                                .color(p.text_secondary),
                        );
                        ui.label(
                            RichText::new(format!("Album: {}", track.album))
                                .size(12.0)
                                .color(p.text_muted),
                        );
                        ui.label(
                            RichText::new(format!("Duration: {}", track.duration_formatted()))
                                .size(12.0)
                                .color(p.text_muted),
                        );

                        if track.is_stream {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("✨ Online stream")
                                    .color(p.badge_text)
                                    .strong(),
                            );
                        } else if let Some(path) = &track.file_path {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("File: {}", path))
                                    .size(11.0)
                                    .color(p.text_muted),
                            );
                        }
                        if let Some(url) = &track.support_url {
                            if ui
                                .button(
                                    RichText::new("❤ Support Artist")
                                        .color(p.badge_text)
                                        .strong(),
                                )
                                .on_hover_text("Open the artist or support link in your browser")
                                .clicked()
                            {
                                support_url_to_open = Some(url.clone());
                            }
                        }
                    });
                } else {
                    ui.group(|ui| {
                        ui.heading("No Track Currently Playing");
                        ui.label("Add local audio files or search for legal streams in Discover.");
                    });
                }
                if let Some(url) = support_url_to_open {
                    if let Err(error) = open::that(&url) {
                        self.show_toast(format!("Could not open support link: {}", error));
                    }
                }

                ui.add_space(10.0);
                let mut q_remove = None;
                let mut q_play = None;
                let mut q_move: Option<(usize, i32)> = None;
                let mut q_clear = false;
                ui.horizontal(|ui| {
                    ui.heading("Up Next");
                    ui.label(format!("({} queued)", self.queue.len()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(!self.queue.is_empty(), egui::Button::new("🗑 Clear queue"))
                            .clicked()
                        {
                            q_clear = true;
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("np_queue")
                    .max_height(140.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if self.queue.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Queue is empty. Add songs with “＋ Queue” in the Library.",
                                )
                                .color(p.text_muted),
                            );
                        }
                        let n = self.queue.len();
                        for (qi, t) in self.queue.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}.", qi + 1));
                                ui.label(shorten_end(&format!("{} — {}", t.title, t.artist), 38));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("✖")
                                            .on_hover_text("Remove from queue")
                                            .clicked()
                                        {
                                            q_remove = Some(qi);
                                        }
                                        if ui
                                            .add_enabled(qi + 1 < n, egui::Button::new("⬇").small())
                                            .clicked()
                                        {
                                            q_move = Some((qi, 1));
                                        }
                                        if ui
                                            .add_enabled(qi > 0, egui::Button::new("⬆").small())
                                            .clicked()
                                        {
                                            q_move = Some((qi, -1));
                                        }
                                        if ui.small_button("▶").on_hover_text("Play now").clicked()
                                        {
                                            q_play = Some(qi);
                                        }
                                    },
                                );
                            });
                        }
                    });
                if q_clear {
                    self.queue.clear();
                    self.show_toast("Queue cleared".to_string());
                }
                if let Some(qi) = q_remove {
                    if qi < self.queue.len() {
                        self.queue.remove(qi);
                    }
                }
                if let Some((qi, d)) = q_move {
                    let to = qi as i32 + d;
                    if to >= 0 && (to as usize) < self.queue.len() {
                        self.queue.swap(qi, to as usize);
                    }
                }
                if let Some(qi) = q_play {
                    if qi < self.queue.len() {
                        let t = self.queue.remove(qi);
                        if let Some(i) = self.playlist.iter().position(|p| p.id == t.id) {
                            self.play_index(i);
                        }
                    }
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.heading("Library Playlist");
                    ui.label(format!("({} songs)", self.playlist.len()));
                });

                // Scrollable list of active tracks
                egui::ScrollArea::vertical()
                    .id_salt("np_playlist")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        let mut play_target = None;
                        let mut enqueue_target = None;
                        for (i, t) in self.playlist.iter().enumerate() {
                            let is_current = self.current_index == Some(i);
                            ui.horizontal(|ui| {
                                let prefix = if is_current { "▶ " } else { "  " };
                                let label_text = shorten_end(
                                    &format!("{}{}. {} - {}", prefix, i + 1, t.artist, t.title),
                                    42,
                                );
                                let response = ui.selectable_label(is_current, label_text);
                                if response.clicked() {
                                    play_target = Some(i);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(t.duration_formatted())
                                                .size(12.0)
                                                .monospace()
                                                .color(p.text_secondary),
                                        );
                                        if ui
                                            .small_button("＋")
                                            .on_hover_text("Add to queue")
                                            .clicked()
                                        {
                                            enqueue_target = Some(i);
                                        }
                                    },
                                );
                            });
                        }
                        if let Some(idx) = play_target {
                            self.play_index(idx);
                        }
                        if let Some(idx) = enqueue_target {
                            self.enqueue(idx, false);
                        }
                    });
            });
        });
    }

    fn render_library_view(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.add_space(4.0);

        // Top Toolbar
        ui.horizontal(|ui| {
            ui.heading("🎵 Music Catalog");
            ui.label(format!("({} tracks indexed)", self.playlist.len()));
            if self.scanning > 0 {
                ui.add(egui::Spinner::new());
                ui.label(RichText::new("Scanning…").color(p.accent));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("🔄 Rescan All")
                    .on_hover_text("Rescan all library folders and update SQLite catalog")
                    .clicked()
                {
                    self.rescan_library();
                }

                if ui
                    .button("🎵 Add Files")
                    .on_hover_text("Select local audio files")
                    .clicked()
                {
                    if let Some(files) = rfd::FileDialog::new()
                        .add_filter(
                            "Audio Files",
                            &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"],
                        )
                        .pick_files()
                    {
                        let mut tracks = Vec::new();
                        for file in files {
                            if let Some(t) = crate::library::load_local_track(&file) {
                                let path_str = file.to_string_lossy().to_string();
                                if !self.saved.files.contains(&path_str) {
                                    self.saved.files.push(path_str);
                                }
                                tracks.push(t);
                            }
                        }
                        crate::library::save_saved(&self.saved);
                        let _ = self.db.upsert_tracks_batch(&tracks);
                        self.db_stats = self.db.get_stats();
                        let added = self.add_tracks(tracks);
                        self.rebuild_folder_tree();
                        self.show_toast(format!("Added {} new track(s)", added));
                    }
                }

                if ui
                    .button("📂 Add Folder")
                    .on_hover_text("Index a directory tree into SQLite")
                    .clicked()
                {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.add_folder(folder);
                    }
                }
            });
        });

        ui.add_space(4.0);

        // Foobar Filter & Search Row
        ui.horizontal_wrapped(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.library_filter)
                    .hint_text("Live filter (title, artist, album, path...)")
                    .desired_width(220.0),
            );

            egui::ComboBox::from_id_salt("filter_field")
                .selected_text(format!("In: {}", self.filter_field.label()))
                .show_ui(ui, |ui| {
                    for f in [
                        FilterField::All,
                        FilterField::Title,
                        FilterField::Artist,
                        FilterField::Album,
                        FilterField::FileName,
                        FilterField::Folder,
                    ] {
                        ui.selectable_value(&mut self.filter_field, f, f.label());
                    }
                });

            egui::ComboBox::from_id_salt("sort_key")
                .selected_text(format!("Sort: {}", self.sort_key.label()))
                .show_ui(ui, |ui| {
                    for s in [
                        SortKey::Added,
                        SortKey::Title,
                        SortKey::Artist,
                        SortKey::Album,
                        SortKey::Duration,
                        SortKey::Folder,
                    ] {
                        ui.selectable_value(&mut self.sort_key, s, s.label());
                    }
                });
            ui.checkbox(&mut self.sort_desc, "Descending");

            if ui.button("✖ Reset").clicked() {
                self.library_filter.clear();
                self.folder_tree.selected_path = None;
                self.album_filter = None;
                self.artist_filter = None;
                self.filter_field = FilterField::All;
                self.sort_key = SortKey::Added;
                self.sort_desc = false;
            }
        });

        ui.add_space(4.0);

        // Main Layout: Foobar-style Sidebar + Virtualized Track Grid
        let avail_size = ui.available_size();
        let sidebar_width = 260.0;
        let right_width = (avail_size.x - sidebar_width - 16.0).max(300.0);

        ui.horizontal_top(|ui| {
            // Sidebar Container
            ui.allocate_ui_with_layout(
                egui::vec2(sidebar_width, avail_size.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_foobar_sidebar(ui, p);
                },
            );

            ui.separator();

            // Right Pane: Active Path Breadcrumbs + Track List
            ui.allocate_ui_with_layout(
                egui::vec2(right_width, avail_size.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_library_breadcrumbs(ui, p);
                    ui.add_space(4.0);
                    self.render_virtualized_track_table(ui, p);
                },
            );
        });
    }

    fn render_foobar_sidebar(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.group(|ui| {
            ui.add_space(2.0);

            // Mode Selector
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.sidebar_mode == SidebarMode::FolderTree, "📁 Tree")
                    .clicked()
                {
                    self.sidebar_mode = SidebarMode::FolderTree;
                }
                if ui
                    .selectable_label(self.sidebar_mode == SidebarMode::Albums, "💿 Albums")
                    .clicked()
                {
                    self.sidebar_mode = SidebarMode::Albums;
                }
                if ui
                    .selectable_label(self.sidebar_mode == SidebarMode::Artists, "👤 Artists")
                    .clicked()
                {
                    self.sidebar_mode = SidebarMode::Artists;
                }
            });

            ui.separator();

            match self.sidebar_mode {
                SidebarMode::FolderTree => {
                    // Tree Controls
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("➕ All")
                            .on_hover_text("Expand all tree folders")
                            .clicked()
                        {
                            self.folder_tree.expand_all();
                        }
                        if ui
                            .small_button("➖ All")
                            .on_hover_text("Collapse all folders")
                            .clicked()
                        {
                            self.folder_tree.collapse_all();
                        }
                        ui.checkbox(&mut self.folder_tree.include_subfolders, "Subfolders");
                    });

                    ui.separator();

                    egui::ScrollArea::both()
                        .id_salt("foobar_folder_scroll")
                        .max_height(ui.available_height() - 10.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let total = self.playlist.len();
                            let is_all_selected = self.folder_tree.selected_path.is_none();
                            if ui
                                .selectable_label(
                                    is_all_selected,
                                    format!("📁 All Folders ({})", total),
                                )
                                .clicked()
                            {
                                self.folder_tree.selected_path = None;
                            }

                            ui.add_space(4.0);

                            let roots = self.folder_tree.roots.clone();
                            for (_, root_node) in roots {
                                self.render_tree_node(ui, &root_node, 0, p);
                            }
                        });
                }
                SidebarMode::Albums => {
                    let mut album_counts: std::collections::BTreeMap<String, usize> =
                        std::collections::BTreeMap::new();
                    for t in &self.playlist {
                        *album_counts.entry(t.album.clone()).or_default() += 1;
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("albums_scroll")
                        .max_height(ui.available_height() - 10.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.album_filter.is_none(),
                                    format!("💿 All Albums ({})", self.playlist.len()),
                                )
                                .clicked()
                            {
                                self.album_filter = None;
                            }
                            for (album, count) in album_counts {
                                let is_selected = self.album_filter.as_deref() == Some(&album);
                                let text = format!("💿 {} ({})", shorten_end(&album, 24), count);
                                if ui.selectable_label(is_selected, text).clicked() {
                                    self.album_filter =
                                        if is_selected { None } else { Some(album) };
                                }
                            }
                        });
                }
                SidebarMode::Artists => {
                    let mut artist_counts: std::collections::BTreeMap<String, usize> =
                        std::collections::BTreeMap::new();
                    for t in &self.playlist {
                        *artist_counts.entry(t.artist.clone()).or_default() += 1;
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("artists_scroll")
                        .max_height(ui.available_height() - 10.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.artist_filter.is_none(),
                                    format!("👤 All Artists ({})", self.playlist.len()),
                                )
                                .clicked()
                            {
                                self.artist_filter = None;
                            }
                            for (artist, count) in artist_counts {
                                let is_selected = self.artist_filter.as_deref() == Some(&artist);
                                let text = format!("👤 {} ({})", shorten_end(&artist, 24), count);
                                if ui.selectable_label(is_selected, text).clicked() {
                                    self.artist_filter =
                                        if is_selected { None } else { Some(artist) };
                                }
                            }
                        });
                }
            }
        });
    }

    fn render_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        node: &FolderTreeNode,
        depth: usize,
        p: &ThemePalette,
    ) {
        let has_children = !node.children.is_empty();
        let is_expanded = self.folder_tree.is_expanded(&node.full_path);
        let is_selected = self.folder_tree.selected_path.as_ref() == Some(&node.full_path);

        ui.horizontal(|ui| {
            if depth > 0 {
                ui.add_space((depth as f32) * 14.0);
            }

            if has_children {
                let arrow = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    self.folder_tree.toggle_expanded(&node.full_path);
                }
            } else {
                ui.add_space(18.0);
            }

            let folder_icon = if is_selected { "📂" } else { "📁" };
            let label_str = format!("{} {} ({})", folder_icon, node.name, node.total_tracks);

            let rich = if is_selected {
                RichText::new(label_str).color(p.accent).strong()
            } else {
                RichText::new(label_str).color(p.text_primary)
            };

            let response = ui.selectable_label(is_selected, rich);
            if response.clicked() {
                self.folder_tree.selected_path = Some(node.full_path.clone());
            }
            response.on_hover_text(node.full_path.display().to_string());
        });

        if is_expanded && has_children {
            for child in node.children.values() {
                self.render_tree_node(ui, child, depth + 1, p);
            }
        }
    }

    fn render_library_breadcrumbs(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("📁 Path:").strong().color(p.accent));

            if let Some(selected_path) = &self.folder_tree.selected_path {
                ui.label(
                    RichText::new(selected_path.display().to_string())
                        .monospace()
                        .color(p.text_primary),
                );

                if ui.button("✖ Clear folder filter").clicked() {
                    self.folder_tree.selected_path = None;
                }
            } else {
                ui.label(
                    RichText::new("All Library Folders")
                        .italics()
                        .color(p.text_muted),
                );
            }

            if let Some(alb) = &self.album_filter {
                ui.label(RichText::new(format!("[Album: {}]", alb)).color(p.badge_text));
                if ui.small_button("✖").clicked() {
                    self.album_filter = None;
                }
            }

            if let Some(art) = &self.artist_filter {
                ui.label(RichText::new(format!("[Artist: {}]", art)).color(p.badge_text));
                if ui.small_button("✖").clicked() {
                    self.artist_filter = None;
                }
            }
        });
    }

    fn render_virtualized_track_table(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        let filter_lower = self.library_filter.trim().to_lowercase();

        // High-performance filter execution
        let mut order: Vec<usize> = (0..self.playlist.len())
            .filter(|&i| {
                let t = &self.playlist[i];

                // Foobar folder traversal match
                if !self.folder_tree.matches_track(t.file_path.as_deref()) {
                    return false;
                }

                // Album filter
                if let Some(ref a) = self.album_filter {
                    if &t.album != a {
                        return false;
                    }
                }

                // Artist filter
                if let Some(ref art) = self.artist_filter {
                    if &t.artist != art {
                        return false;
                    }
                }

                // Instant text filter
                if filter_lower.is_empty() {
                    return true;
                }

                let has = |s: &str| s.to_lowercase().contains(&filter_lower);
                match self.filter_field {
                    FilterField::All => {
                        has(&t.title)
                            || has(&t.artist)
                            || has(&t.album)
                            || has(&t.file_name())
                            || has(&t.folder())
                    }
                    FilterField::Title => has(&t.title),
                    FilterField::Artist => has(&t.artist),
                    FilterField::Album => has(&t.album),
                    FilterField::FileName => has(&t.file_name()),
                    FilterField::Folder => has(&t.folder()),
                }
            })
            .collect();

        // Sort
        {
            let pl = &self.playlist;
            match self.sort_key {
                SortKey::Added => {}
                SortKey::Title => order.sort_by_key(|&i| pl[i].title.to_lowercase()),
                SortKey::Artist => order.sort_by_key(|&i| pl[i].artist.to_lowercase()),
                SortKey::Album => order.sort_by_key(|&i| pl[i].album.to_lowercase()),
                SortKey::Folder => order.sort_by_key(|&i| pl[i].folder().to_lowercase()),
                SortKey::Duration => {
                    order.sort_by(|&a, &b| pl[a].duration.total_cmp(&pl[b].duration))
                }
            }
            if self.sort_desc {
                order.reverse();
            }
        }

        ui.label(
            RichText::new(format!(
                "Showing {} of {} tracks",
                order.len(),
                self.playlist.len()
            ))
            .size(12.0)
            .color(p.text_muted),
        );

        let mut remove_index = None;
        let mut play_index = None;
        let mut queue_add: Option<(usize, bool)> = None;

        if self.playlist.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.heading("Library is Empty");
                ui.label("Click 'Add Folder' to index music with the SQLite engine.");
                ui.add_space(10.0);
                if ui.button("Load Built-in Demo Tracks").clicked() {
                    self.load_initial_tracks();
                }
            });
            return;
        }

        if order.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label("No tracks match your current filter or folder selection.")
            });
            return;
        }

        // Table Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("  #").strong().color(p.text_secondary));
            ui.add_space(20.0);
            ui.label(RichText::new("Title").strong().color(p.text_primary));
            ui.add_space(180.0);
            ui.label(RichText::new("Artist").strong().color(p.text_primary));
            ui.add_space(100.0);
            ui.label(RichText::new("Album").strong().color(p.text_primary));
            ui.add_space(80.0);
            ui.label(RichText::new("Duration").strong().color(p.text_primary));
        });
        ui.separator();

        // PERFORMANCE FIX: Virtualized Row Scrolling (Only renders visible rows!)
        let row_height = 28.0;
        let num_rows = order.len();

        egui::ScrollArea::vertical()
            .id_salt("virtual_library_scroll")
            .auto_shrink([false, false])
            .show_rows(ui, row_height, num_rows, |ui, row_range| {
                for row_idx in row_range {
                    let i = order[row_idx];
                    let track = &self.playlist[i];
                    let is_current = self.current_index == Some(i);

                    let row_fill = if is_current {
                        p.row_playing_bg
                    } else if row_idx % 2 == 0 {
                        p.row_item_bg
                    } else {
                        p.bg_card
                    };

                    egui::Frame::new()
                        .fill(row_fill)
                        .inner_margin(egui::Margin::symmetric(6, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Index / Playing Indicator
                                if is_current {
                                    ui.label(RichText::new("▶").color(p.accent).strong());
                                } else {
                                    ui.label(
                                        RichText::new(format!("{:3}", row_idx + 1))
                                            .monospace()
                                            .color(p.text_muted),
                                    );
                                }

                                ui.add_space(8.0);

                                // Title
                                let title_text = shorten_end(&track.title, 28);
                                let title_rich = if is_current {
                                    RichText::new(title_text).color(p.accent).strong()
                                } else {
                                    RichText::new(title_text).color(p.text_primary)
                                };
                                let r = ui.selectable_label(is_current, title_rich);
                                if r.clicked() {
                                    play_index = Some(i);
                                }
                                r.on_hover_text(&track.title);

                                ui.add_space(8.0);

                                // Artist
                                ui.label(
                                    RichText::new(shorten_end(&track.artist, 20))
                                        .color(p.text_secondary),
                                );

                                ui.add_space(8.0);

                                // Album
                                ui.label(
                                    RichText::new(shorten_end(&track.album, 18))
                                        .color(p.text_muted),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("🗑")
                                            .on_hover_text("Remove from catalog")
                                            .clicked()
                                        {
                                            remove_index = Some(i);
                                        }
                                        if ui.small_button("⏭").on_hover_text("Play next").clicked()
                                        {
                                            queue_add = Some((i, true));
                                        }
                                        if ui
                                            .small_button("＋")
                                            .on_hover_text("Add to queue")
                                            .clicked()
                                        {
                                            queue_add = Some((i, false));
                                        }
                                        if ui
                                            .small_button("▶")
                                            .on_hover_text("Play track")
                                            .clicked()
                                        {
                                            play_index = Some(i);
                                        }
                                        ui.label(
                                            RichText::new(track.duration_formatted())
                                                .monospace()
                                                .size(12.0)
                                                .color(p.text_secondary),
                                        );
                                    },
                                );
                            });
                        });
                }
            });

        if let Some((i, front)) = queue_add {
            self.enqueue(i, front);
        }
        if let Some(idx) = play_index {
            self.play_index(idx);
        }
        if let Some(idx) = remove_index {
            self.remove_track(idx);
        }
    }

    fn render_discover_view(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.add_space(8.0);
        ui.heading("🔎 Discover / Providers");
        ui.label("Search Audius or run an external provider configured by you. Downloads are shown only when the source explicitly permits them.");
        ui.add_space(8.0);

        if let Some(error) = &self.provider_config_error {
            ui.colored_label(p.accent, format!("Provider configuration: {}", error));
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Source:");
                let selected = self
                    .active_provider
                    .and_then(|index| self.providers.get(index))
                    .map(|provider| provider.name.as_str())
                    .unwrap_or("Audius");
                egui::ComboBox::from_id_salt("discover_source")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.active_provider.is_none(), "Audius")
                            .clicked()
                        {
                            self.active_provider = None;
                        }
                        for (index, provider) in self.providers.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    self.active_provider == Some(index),
                                    &provider.name,
                                )
                                .clicked()
                            {
                                self.active_provider = Some(index);
                            }
                        }
                    });

                let text_edit = ui.add_sized(
                    [ui.available_width() - 180.0, 26.0],
                    egui::TextEdit::singleline(&mut self.provider_search_input)
                        .hint_text("Search for an artist, track, or provider URL"),
                );
                if text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.start_discover_search();
                }
                if self.discover_is_searching {
                    ui.add(egui::Spinner::new());
                } else if ui.button("🔍 Search").clicked() {
                    self.start_discover_search();
                }
            });
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Status:").strong());
            ui.label(RichText::new(&self.discover_status_message).color(p.accent));
            if self.discover_is_downloading {
                ui.add(egui::Spinner::new());
            }
        });

        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !self.audius_results.is_empty() {
                ui.heading("Audius results");
                let mut stream_target = None;
                let mut download_target = None;
                for item in &self.audius_results {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&item.title).strong().color(p.text_primary));
                                ui.label(format!(
                                    "{} · {}",
                                    item.artist,
                                    item.duration_formatted()
                                ));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if item.is_downloadable
                                        && ui.button("📥 Download to Library").clicked()
                                    {
                                        download_target = Some(item.clone());
                                    }
                                    if ui.button("▶ Stream").clicked() {
                                        stream_target = Some(item.clone());
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(4.0);
                }
                if let Some(item) = stream_target {
                    self.start_audius_stream(item);
                }
                if let Some(item) = download_target {
                    self.start_audius_download(item);
                }
            }

            if !self.provider_results.is_empty() {
                ui.heading("Provider results");
                let mut stream_target = None;
                let mut download_target = None;
                let provider = self
                    .active_provider
                    .and_then(|index| self.providers.get(index).cloned());
                for item in &self.provider_results {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&item.title).strong().color(p.text_primary));
                                ui.label(format!(
                                    "{} · {}",
                                    item.artist,
                                    item.duration_formatted()
                                ));
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if item.downloadable
                                        && ui.button("📥 Download to Library").clicked()
                                    {
                                        download_target = Some(item.clone());
                                    }
                                    if ui.button("▶ Stream").clicked() {
                                        stream_target = Some(item.clone());
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(4.0);
                }
                if let Some((item, provider)) = stream_target.zip(provider.clone()) {
                    self.start_provider_stream(provider, item);
                }
                if let Some((item, provider)) = download_target.zip(provider) {
                    self.start_provider_download(provider, item);
                }
            }

            if self.audius_results.is_empty()
                && self.provider_results.is_empty()
                && !self.discover_is_searching
            {
                ui.vertical_centered(|ui| {
                    ui.label("Choose a source and enter a search query to discover music.");
                    ui.label(format!(
                        "Provider configuration: {}",
                        providers::config_path().display()
                    ));
                });
            }

            if !self.downloaded_history.is_empty() {
                ui.add_space(16.0);
                ui.heading("Recently downloaded");
                for track in &self.downloaded_history {
                    ui.horizontal(|ui| {
                        ui.label("✔");
                        ui.label(RichText::new(&track.title).strong());
                        ui.label(format!("- {}", track.artist));
                    });
                }
            }
        });
    }

    fn render_settings_view(&mut self, ui: &mut egui::Ui, p: &ThemePalette) {
        ui.add_space(8.0);
        ui.heading("⚙ Settings & Advanced Media Engine Diagnostics");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Theme & Visual Styles
            ui.group(|ui| {
                ui.heading("🎨 Theme & Visual Display");
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Select Theme:");
                    for theme in PlayerTheme::all() {
                        let is_current = self.current_theme == *theme;
                        if ui.selectable_label(is_current, theme.label()).clicked() {
                            self.set_theme(*theme);
                        }
                    }
                });

                ui.add_space(6.0);
                ui.label(RichText::new(format!("Current: {} — {}", p.name, p.description)).color(p.text_secondary));

                ui.add_space(8.0);
                if ui.checkbox(&mut self.crt_scanlines, "Enable Retro CRT Scanlines & Cathode Ray Overlay").changed() {
                    let _ = self.db.set_setting("crt_scanlines", if self.crt_scanlines { "true" } else { "false" });
                }
            });

            ui.add_space(10.0);

            // SQLite Media Catalog Database
            ui.group(|ui| {
                ui.heading("🗄 SQLite Media Catalog (WAL Mode)");
                ui.add_space(6.0);

                ui.label(format!("Database Location: {}", self.db.db_path.display()));
                ui.label(format!("Database File Size: {:.2} KB", self.db_stats.db_file_size_bytes as f32 / 1024.0));
                ui.label(format!("Indexed Tracks: {}", self.db_stats.total_tracks));
                ui.label(format!("Distinct Artists: {}", self.db_stats.total_artists));
                ui.label(format!("Distinct Albums: {}", self.db_stats.total_albums));
                ui.label(format!("Total Catalog Audio Duration: {}", format_time(self.db_stats.total_duration_secs)));
                ui.label(format!("Total Track Playbacks: {}", self.db_stats.total_plays));

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("⚡ Vacuum & Optimize SQLite DB").clicked() {
                        if self.db.vacuum().is_ok() {
                            self.db_stats = self.db.get_stats();
                            self.show_toast("SQLite database vacuumed and optimized!".to_string());
                        }
                    }

                    if ui.button("🔄 Rescan All Catalog Tracks").clicked() {
                        self.rescan_library();
                    }

                    if ui.button("🗑 Clear SQLite Catalog").clicked() {
                        if self.db.clear_all_tracks().is_ok() {
                            self.playlist.clear();
                            self.rebuild_folder_tree();
                            self.db_stats = self.db.get_stats();
                            self.show_toast("SQLite catalog cleared.".to_string());
                        }
                    }
                });
            });

            ui.add_space(10.0);

            // Managed Library Folders
            ui.group(|ui| {
                ui.heading("📂 Managed Music Directories");
                ui.add_space(6.0);

                ui.label(format!("Local Library Root: {}", self.library_dir.display()));
                ui.label(format!("Provider Config: {}", providers::config_path().display()));

                let mut to_remove = None;
                if !self.saved.folders.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new("External Indexed Folders:").strong());
                    for folder in &self.saved.folders {
                        ui.horizontal(|ui| {
                            ui.label(format!("📁 {}", folder));
                            if ui.small_button("🗑 Remove").clicked() {
                                to_remove = Some(folder.clone());
                            }
                        });
                    }
                }

                if let Some(f) = to_remove {
                    self.remove_folder_from_library(&f);
                }

                ui.add_space(8.0);
                if ui.button("📂 Add New Folder to Library").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.add_folder(folder);
                    }
                }
            });

            ui.add_space(10.0);

            // Performance & Audio Engine Diagnostics
            ui.group(|ui| {
                ui.heading("⚡ Performance & Audio Backend");
                ui.add_space(6.0);
                ui.label("Rendering Engine: eframe 0.31 with Adaptive 60 FPS Event Limiter (Zero CPU Lag)");
                ui.label("Audio Output Backend: CPAL / Rodio (Low-latency ALSA / PulseAudio)");
                ui.label("Seek Engine: Precision time-indexed seek with fallback frame reconstruction");
                ui.label("Metadata Backend: Pure-Rust lofty tag and audio-property reader");
                ui.label("External Providers: Optional user-configured command hooks");
            });
        });
    }
}

fn shorten_end(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 2).collect();
    format!("{}...", head)
}
