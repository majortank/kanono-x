use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlayerTheme {
    #[default]
    CrtGreen,
    CrtAmber,
    CrtCyan,
    OmarchyEmerald,
    OmarchyCyberpunk,
    OmarchyNord,
    OmarchyDracula,
    SolarizedLight,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub name: &'static str,
    pub description: &'static str,
    pub is_dark: bool,
    pub is_crt: bool,
    pub bg_root: Color32,
    pub bg_panel: Color32,
    pub bg_card: Color32,
    pub bg_card_hover: Color32,
    pub border: Color32,
    pub border_subtle: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_text: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub nav_selected_bg: Color32,
    pub nav_selected_border: Color32,
    pub row_item_bg: Color32,
    pub row_hover_bg: Color32,
    pub row_selected_bg: Color32,
    pub row_playing_bg: Color32,
    pub badge_bg: Color32,
    pub badge_text: Color32,
    pub visualizer_low: Color32,
    pub visualizer_mid: Color32,
    pub visualizer_high: Color32,
}

impl PlayerTheme {
    pub fn all() -> &'static [PlayerTheme] {
        &[
            PlayerTheme::CrtGreen,
            PlayerTheme::CrtAmber,
            PlayerTheme::CrtCyan,
            PlayerTheme::OmarchyEmerald,
            PlayerTheme::OmarchyCyberpunk,
            PlayerTheme::OmarchyNord,
            PlayerTheme::OmarchyDracula,
            PlayerTheme::SolarizedLight,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::CrtGreen => "📺 CRT Green Phosphor",
            Self::CrtAmber => "📺 CRT Amber Terminal",
            Self::CrtCyan => "📺 CRT Cyber Cyan",
            Self::OmarchyEmerald => "🌿 Omarchy Obsidian Emerald",
            Self::OmarchyCyberpunk => "⚡ Omarchy Cyberpunk Neon",
            Self::OmarchyNord => "❄ Omarchy Nord Frost",
            Self::OmarchyDracula => "🦇 Omarchy Dracula",
            Self::SolarizedLight => "☀ Solarized Daylight",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::CrtGreen => Self::CrtAmber,
            Self::CrtAmber => Self::CrtCyan,
            Self::CrtCyan => Self::OmarchyEmerald,
            Self::OmarchyEmerald => Self::OmarchyCyberpunk,
            Self::OmarchyCyberpunk => Self::OmarchyNord,
            Self::OmarchyNord => Self::OmarchyDracula,
            Self::OmarchyDracula => Self::SolarizedLight,
            Self::SolarizedLight => Self::CrtGreen,
        }
    }

    pub fn palette(&self) -> ThemePalette {
        match self {
            Self::CrtGreen => ThemePalette {
                name: "CRT Green Phosphor",
                description:
                    "Authentic P1 phosphor monochrome CRT green terminal with intense glow",
                is_dark: true,
                is_crt: true,
                bg_root: Color32::from_rgb(6, 12, 8),
                bg_panel: Color32::from_rgb(10, 20, 13),
                bg_card: Color32::from_rgb(14, 28, 18),
                bg_card_hover: Color32::from_rgb(20, 42, 26),
                border: Color32::from_rgb(34, 197, 94),
                border_subtle: Color32::from_rgb(18, 64, 32),
                accent: Color32::from_rgb(74, 222, 128),
                accent_hover: Color32::from_rgb(34, 197, 94),
                accent_text: Color32::from_rgb(6, 16, 8),
                text_primary: Color32::from_rgb(187, 247, 208),
                text_secondary: Color32::from_rgb(134, 239, 172),
                text_muted: Color32::from_rgb(52, 120, 75),
                nav_selected_bg: Color32::from_rgb(16, 68, 32),
                nav_selected_border: Color32::from_rgb(74, 222, 128),
                row_item_bg: Color32::from_rgb(9, 18, 12),
                row_hover_bg: Color32::from_rgb(18, 38, 24),
                row_selected_bg: Color32::from_rgb(22, 54, 30),
                row_playing_bg: Color32::from_rgb(15, 60, 28),
                badge_bg: Color32::from_rgb(14, 46, 22),
                badge_text: Color32::from_rgb(74, 222, 128),
                visualizer_low: Color32::from_rgb(34, 197, 94),
                visualizer_mid: Color32::from_rgb(74, 222, 128),
                visualizer_high: Color32::from_rgb(187, 247, 208),
            },
            Self::CrtAmber => ThemePalette {
                name: "CRT Amber Terminal",
                description:
                    "Warm P3 amber CRT cathode tube glow from vintage industrial workstations",
                is_dark: true,
                is_crt: true,
                bg_root: Color32::from_rgb(14, 10, 4),
                bg_panel: Color32::from_rgb(24, 16, 8),
                bg_card: Color32::from_rgb(34, 24, 12),
                bg_card_hover: Color32::from_rgb(50, 34, 18),
                border: Color32::from_rgb(245, 158, 11),
                border_subtle: Color32::from_rgb(80, 50, 20),
                accent: Color32::from_rgb(251, 191, 36),
                accent_hover: Color32::from_rgb(245, 158, 11),
                accent_text: Color32::from_rgb(20, 12, 4),
                text_primary: Color32::from_rgb(254, 243, 199),
                text_secondary: Color32::from_rgb(253, 230, 138),
                text_muted: Color32::from_rgb(146, 100, 48),
                nav_selected_bg: Color32::from_rgb(70, 42, 14),
                nav_selected_border: Color32::from_rgb(251, 191, 36),
                row_item_bg: Color32::from_rgb(18, 12, 6),
                row_hover_bg: Color32::from_rgb(38, 26, 14),
                row_selected_bg: Color32::from_rgb(58, 38, 18),
                row_playing_bg: Color32::from_rgb(64, 40, 16),
                badge_bg: Color32::from_rgb(50, 32, 12),
                badge_text: Color32::from_rgb(251, 191, 36),
                visualizer_low: Color32::from_rgb(245, 158, 11),
                visualizer_mid: Color32::from_rgb(251, 191, 36),
                visualizer_high: Color32::from_rgb(254, 240, 138),
            },
            Self::CrtCyan => ThemePalette {
                name: "CRT Cyber Cyan",
                description: "Deep oceanic void with radioactive cyan beam vectors",
                is_dark: true,
                is_crt: true,
                bg_root: Color32::from_rgb(4, 12, 16),
                bg_panel: Color32::from_rgb(8, 20, 28),
                bg_card: Color32::from_rgb(12, 30, 42),
                bg_card_hover: Color32::from_rgb(18, 45, 62),
                border: Color32::from_rgb(6, 182, 212),
                border_subtle: Color32::from_rgb(18, 56, 76),
                accent: Color32::from_rgb(34, 211, 238),
                accent_hover: Color32::from_rgb(6, 182, 212),
                accent_text: Color32::from_rgb(4, 16, 24),
                text_primary: Color32::from_rgb(207, 250, 254),
                text_secondary: Color32::from_rgb(165, 243, 252),
                text_muted: Color32::from_rgb(40, 110, 130),
                nav_selected_bg: Color32::from_rgb(12, 58, 80),
                nav_selected_border: Color32::from_rgb(34, 211, 238),
                row_item_bg: Color32::from_rgb(6, 16, 22),
                row_hover_bg: Color32::from_rgb(14, 34, 48),
                row_selected_bg: Color32::from_rgb(18, 50, 70),
                row_playing_bg: Color32::from_rgb(14, 52, 72),
                badge_bg: Color32::from_rgb(12, 44, 60),
                badge_text: Color32::from_rgb(34, 211, 238),
                visualizer_low: Color32::from_rgb(6, 182, 212),
                visualizer_mid: Color32::from_rgb(34, 211, 238),
                visualizer_high: Color32::from_rgb(165, 243, 252),
            },
            Self::OmarchyEmerald => ThemePalette {
                name: "Omarchy Obsidian Emerald",
                description: "Signature Omarchy dark obsidian slate with luminous emerald green",
                is_dark: true,
                is_crt: false,
                bg_root: Color32::from_rgb(11, 15, 23),
                bg_panel: Color32::from_rgb(18, 24, 38),
                bg_card: Color32::from_rgb(24, 32, 50),
                bg_card_hover: Color32::from_rgb(32, 42, 65),
                border: Color32::from_rgb(38, 50, 75),
                border_subtle: Color32::from_rgb(28, 38, 58),
                accent: Color32::from_rgb(16, 185, 129),
                accent_hover: Color32::from_rgb(5, 150, 105),
                accent_text: Color32::WHITE,
                text_primary: Color32::WHITE,
                text_secondary: Color32::from_rgb(148, 163, 184),
                text_muted: Color32::from_rgb(100, 116, 139),
                nav_selected_bg: Color32::from_rgb(6, 78, 59),
                nav_selected_border: Color32::from_rgb(16, 185, 129),
                row_item_bg: Color32::from_rgb(16, 20, 29),
                row_hover_bg: Color32::from_rgb(26, 34, 49),
                row_selected_bg: Color32::from_rgb(30, 48, 80),
                row_playing_bg: Color32::from_rgb(6, 78, 59),
                badge_bg: Color32::from_rgb(30, 41, 59),
                badge_text: Color32::from_rgb(52, 211, 153),
                visualizer_low: Color32::from_rgb(16, 185, 129),
                visualizer_mid: Color32::from_rgb(6, 182, 212),
                visualizer_high: Color32::from_rgb(245, 158, 11),
            },
            Self::OmarchyCyberpunk => ThemePalette {
                name: "Omarchy Cyberpunk Neon",
                description: "Obsidian void infused with neon purple, electric magenta, and cyan",
                is_dark: true,
                is_crt: false,
                bg_root: Color32::from_rgb(10, 8, 18),
                bg_panel: Color32::from_rgb(18, 14, 32),
                bg_card: Color32::from_rgb(26, 20, 46),
                bg_card_hover: Color32::from_rgb(38, 30, 66),
                border: Color32::from_rgb(65, 42, 110),
                border_subtle: Color32::from_rgb(42, 28, 75),
                accent: Color32::from_rgb(168, 85, 247),
                accent_hover: Color32::from_rgb(147, 51, 234),
                accent_text: Color32::WHITE,
                text_primary: Color32::WHITE,
                text_secondary: Color32::from_rgb(196, 181, 253),
                text_muted: Color32::from_rgb(139, 124, 180),
                nav_selected_bg: Color32::from_rgb(88, 28, 135),
                nav_selected_border: Color32::from_rgb(168, 85, 247),
                row_item_bg: Color32::from_rgb(18, 13, 30),
                row_hover_bg: Color32::from_rgb(34, 25, 60),
                row_selected_bg: Color32::from_rgb(60, 25, 95),
                row_playing_bg: Color32::from_rgb(88, 28, 135),
                badge_bg: Color32::from_rgb(45, 25, 80),
                badge_text: Color32::from_rgb(236, 72, 153),
                visualizer_low: Color32::from_rgb(168, 85, 247),
                visualizer_mid: Color32::from_rgb(236, 72, 153),
                visualizer_high: Color32::from_rgb(6, 182, 212),
            },
            Self::OmarchyNord => ThemePalette {
                name: "Omarchy Nord Frost",
                description:
                    "Cool Arctic palette with Nord dark slate and glacial ice blue accents",
                is_dark: true,
                is_crt: false,
                bg_root: Color32::from_rgb(36, 41, 51),
                bg_panel: Color32::from_rgb(46, 52, 64),
                bg_card: Color32::from_rgb(59, 66, 82),
                bg_card_hover: Color32::from_rgb(67, 76, 94),
                border: Color32::from_rgb(76, 86, 106),
                border_subtle: Color32::from_rgb(60, 68, 84),
                accent: Color32::from_rgb(136, 192, 208),
                accent_hover: Color32::from_rgb(129, 161, 193),
                accent_text: Color32::from_rgb(46, 52, 64),
                text_primary: Color32::from_rgb(236, 239, 244),
                text_secondary: Color32::from_rgb(216, 222, 233),
                text_muted: Color32::from_rgb(142, 156, 178),
                nav_selected_bg: Color32::from_rgb(43, 76, 100),
                nav_selected_border: Color32::from_rgb(136, 192, 208),
                row_item_bg: Color32::from_rgb(40, 46, 58),
                row_hover_bg: Color32::from_rgb(54, 62, 78),
                row_selected_bg: Color32::from_rgb(64, 76, 96),
                row_playing_bg: Color32::from_rgb(43, 76, 100),
                badge_bg: Color32::from_rgb(54, 65, 85),
                badge_text: Color32::from_rgb(143, 188, 187),
                visualizer_low: Color32::from_rgb(136, 192, 208),
                visualizer_mid: Color32::from_rgb(129, 161, 193),
                visualizer_high: Color32::from_rgb(163, 190, 140),
            },
            Self::OmarchyDracula => ThemePalette {
                name: "Omarchy Dracula",
                description: "Vampiric dark theme with neon pink, purple, and radioactive mint",
                is_dark: true,
                is_crt: false,
                bg_root: Color32::from_rgb(33, 34, 44),
                bg_panel: Color32::from_rgb(40, 42, 54),
                bg_card: Color32::from_rgb(52, 55, 70),
                bg_card_hover: Color32::from_rgb(68, 71, 90),
                border: Color32::from_rgb(85, 88, 110),
                border_subtle: Color32::from_rgb(60, 62, 80),
                accent: Color32::from_rgb(255, 121, 198),
                accent_hover: Color32::from_rgb(240, 100, 180),
                accent_text: Color32::WHITE,
                text_primary: Color32::from_rgb(248, 248, 242),
                text_secondary: Color32::from_rgb(189, 147, 249),
                text_muted: Color32::from_rgb(130, 135, 155),
                nav_selected_bg: Color32::from_rgb(70, 40, 75),
                nav_selected_border: Color32::from_rgb(255, 121, 198),
                row_item_bg: Color32::from_rgb(38, 40, 52),
                row_hover_bg: Color32::from_rgb(58, 62, 80),
                row_selected_bg: Color32::from_rgb(70, 50, 85),
                row_playing_bg: Color32::from_rgb(50, 75, 60),
                badge_bg: Color32::from_rgb(48, 45, 65),
                badge_text: Color32::from_rgb(80, 250, 123),
                visualizer_low: Color32::from_rgb(80, 250, 123),
                visualizer_mid: Color32::from_rgb(189, 147, 249),
                visualizer_high: Color32::from_rgb(255, 121, 198),
            },
            Self::SolarizedLight => ThemePalette {
                name: "Solarized Daylight",
                description: "Clean daylight high-contrast palette with sapphire blue accents",
                is_dark: false,
                is_crt: false,
                bg_root: Color32::from_rgb(241, 245, 249),
                bg_panel: Color32::from_rgb(255, 255, 255),
                bg_card: Color32::from_rgb(248, 250, 252),
                bg_card_hover: Color32::from_rgb(235, 240, 248),
                border: Color32::from_rgb(203, 213, 225),
                border_subtle: Color32::from_rgb(226, 232, 240),
                accent: Color32::from_rgb(37, 99, 235),
                accent_hover: Color32::from_rgb(29, 78, 216),
                accent_text: Color32::WHITE,
                text_primary: Color32::from_rgb(15, 23, 42),
                text_secondary: Color32::from_rgb(71, 85, 105),
                text_muted: Color32::from_rgb(148, 163, 184),
                nav_selected_bg: Color32::from_rgb(219, 234, 254),
                nav_selected_border: Color32::from_rgb(37, 99, 235),
                row_item_bg: Color32::from_rgb(255, 255, 255),
                row_hover_bg: Color32::from_rgb(241, 245, 249),
                row_selected_bg: Color32::from_rgb(224, 238, 255),
                row_playing_bg: Color32::from_rgb(220, 252, 231),
                badge_bg: Color32::from_rgb(226, 232, 240),
                badge_text: Color32::from_rgb(15, 118, 110),
                visualizer_low: Color32::from_rgb(37, 99, 235),
                visualizer_mid: Color32::from_rgb(13, 148, 136),
                visualizer_high: Color32::from_rgb(234, 88, 12),
            },
        }
    }

    pub fn apply_to_egui(&self, ctx: &egui::Context) {
        let p = self.palette();
        let mut visuals = if p.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        let radius = if p.is_crt {
            CornerRadius::same(2)
        } else {
            CornerRadius::same(6)
        };

        visuals.window_fill = p.bg_panel;
        visuals.panel_fill = p.bg_panel;
        visuals.faint_bg_color = p.bg_root;
        visuals.extreme_bg_color = p.bg_root;

        visuals.widgets.noninteractive.bg_fill = p.bg_card;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
        visuals.widgets.noninteractive.corner_radius = radius;

        visuals.widgets.inactive.bg_fill = p.bg_card;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
        visuals.widgets.inactive.corner_radius = radius;

        visuals.widgets.hovered.bg_fill = p.bg_card_hover;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.accent);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
        visuals.widgets.hovered.corner_radius = radius;

        visuals.widgets.active.bg_fill = p.nav_selected_bg;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.accent_text);
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.accent);
        visuals.widgets.active.corner_radius = radius;

        visuals.widgets.open.bg_fill = p.bg_card_hover;
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);
        visuals.widgets.open.corner_radius = radius;

        visuals.selection.bg_fill = p.nav_selected_bg;
        visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);

        visuals.window_stroke = Stroke::new(1.0_f32, p.border);

        ctx.set_visuals(visuals);
    }
}
