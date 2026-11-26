mod audio;
mod audius;
mod folder_tree;
mod library;
mod library_db;
mod providers;
mod theme;
mod tray;
mod ui;

use eframe::egui;
use std::env;
use tray::TrayController;
use ui::{KanonoApp, ViewTab};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut screenshot_path = None;
    let mut initial_tab = ViewTab::NowPlaying;
    let mut start_hidden = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot_path = Some(args[i + 1].clone());
            i += 1;
        } else if args[i] == "--start-hidden" {
            start_hidden = true;
        } else if args[i] == "--tab" && i + 1 < args.len() {
            match args[i + 1].to_lowercase().as_str() {
                "library" => initial_tab = ViewTab::Library,
                "discover" | "providers" => initial_tab = ViewTab::Discover,
                "settings" => initial_tab = ViewTab::Settings,
                _ => initial_tab = ViewTab::NowPlaying,
            }
            i += 1;
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1160.0, 780.0])
        .with_min_inner_size([800.0, 560.0])
        .with_title("Kanono X — Advanced Hi-Fi Audio Player & Foobar Engine")
        .with_visible(!start_hidden);

    if screenshot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let tray = match TrayController::new() {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("Tray integration unavailable: {error}");
            None
        }
    };
    eframe::run_native(
        "Kanono X",
        options,
        Box::new(move |cc| {
            let mut app = KanonoApp::new(screenshot_path, tray);
            app.current_tab = initial_tab;
            app.current_theme.apply_to_egui(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
