use crate::audio::AudioEngine;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIconBuilder,
};

pub struct TrayController {
    _tray: tray_icon::TrayIcon,
    show_hide: MenuItem,
    play_pause: MenuItem,
    previous: MenuItem,
    next: MenuItem,
    quit: MenuItem,
}

pub enum TrayAction {
    ShowHide,
    PlayPause,
    Previous,
    Next,
    Quit,
}

impl TrayController {
    pub fn new() -> Result<Self, String> {
        let menu = Menu::new();
        let show_hide = MenuItem::new("Show / Hide Kanono X", true, None);
        let play_pause = MenuItem::new("Play / Pause", true, None);
        let previous = MenuItem::new("Previous track", true, None);
        let next = MenuItem::new("Next track", true, None);
        let quit = MenuItem::new("Quit", true, None);

        menu.append_items(&[&show_hide, &play_pause, &previous, &next, &quit])
            .map_err(|error| format!("Could not create tray menu: {error}"))?;

        let icon = Icon::from_rgba(tray_icon_rgba(), 32, 32)
            .map_err(|error| format!("Could not create tray icon: {error}"))?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("Kanono X")
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .build()
            .map_err(|error| format!("Could not create tray icon: {error}"))?;

        Ok(Self {
            _tray: tray,
            show_hide,
            play_pause,
            previous,
            next,
            quit,
        })
    }

    pub fn action(&self) -> Option<TrayAction> {
        let event = MenuEvent::receiver().try_iter().last()?;
        if event.id == *self.show_hide.id() {
            Some(TrayAction::ShowHide)
        } else if event.id == *self.play_pause.id() {
            Some(TrayAction::PlayPause)
        } else if event.id == *self.previous.id() {
            Some(TrayAction::Previous)
        } else if event.id == *self.next.id() {
            Some(TrayAction::Next)
        } else if event.id == *self.quit.id() {
            Some(TrayAction::Quit)
        } else {
            None
        }
    }
}

pub fn update_tooltip(tray: &TrayController, audio: &AudioEngine) {
    let title = audio
        .current_track
        .as_ref()
        .map(|track| track.title.as_str())
        .unwrap_or("Nothing playing");
    let _ = tray._tray.set_tooltip(Some(&format!("Kanono X — {title}")));
}

fn tray_icon_rgba() -> Vec<u8> {
    let mut pixels = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let index = (y * 32 + x) * 4;
            if dx * dx + dy * dy <= 14 * 14 {
                pixels[index..index + 4].copy_from_slice(&[42, 221, 161, 255]);
            }
        }
    }
    pixels
}
