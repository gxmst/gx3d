use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    SimplifiedChinese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionOption {
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
}

pub const RESOLUTION_OPTIONS: [ResolutionOption; 5] = [
    ResolutionOption {
        label: "1280 x 720",
        width: 1280,
        height: 720,
    },
    ResolutionOption {
        label: "1600 x 900",
        width: 1600,
        height: 900,
    },
    ResolutionOption {
        label: "1920 x 1080",
        width: 1920,
        height: 1080,
    },
    ResolutionOption {
        label: "2560 x 1440",
        width: 2560,
        height: 1440,
    },
    ResolutionOption {
        label: "3440 x 1440 (21:9)",
        width: 3440,
        height: 1440,
    },
];

/// Startup default for god mode. The menu checkbox (`PauseMenu`) and the
/// V-key handler (`ViewModeState`) both read this so they can never disagree
/// before the first toggle.
pub const DEFAULT_GOD_MODE_ENABLED: bool = true;

#[derive(Debug, Clone)]
pub struct PauseMenu {
    pub open: bool,
    pub language: Language,
    pub selected_resolution: usize,
    pub god_mode_enabled: bool,
    /// Index into `core::config::SENSITIVITY_OPTIONS`.
    pub sensitivity_index: usize,
    /// Selected weather mode shown on the weather button.
    pub weather: crate::game::systems::weather::WeatherKind,
    /// Scene names shown by the scene selector (from `SceneLibrary`).
    pub scene_names: Vec<String>,
    /// Which entry corresponds to the loaded scene (highlighted).
    pub current_scene: Option<usize>,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            language: Language::SimplifiedChinese,
            selected_resolution: 2,
            god_mode_enabled: DEFAULT_GOD_MODE_ENABLED,
            sensitivity_index: crate::core::config::DEFAULT_SENSITIVITY_INDEX,
            weather: Default::default(),
            scene_names: Vec::new(),
            current_scene: None,
        }
    }

    pub fn hit_test(&self, width: f32, height: f32, cursor: Vec2) -> Option<MenuAction> {
        let layout = PauseMenuLayout::new_with_scenes(width, height, self.scene_names.len());
        for button in layout.resolution_buttons {
            if button.rect.contains(cursor) {
                return Some(MenuAction::SetResolution(button.index));
            }
        }
        for button in layout.scene_buttons {
            if button.rect.contains(cursor) {
                return Some(MenuAction::LoadScene(button.index));
            }
        }
        if layout.sensitivity_button.contains(cursor) {
            return Some(MenuAction::CycleSensitivity);
        }
        if layout.weather_button.contains(cursor) {
            return Some(MenuAction::CycleWeather);
        }
        if layout.language_button.contains(cursor) {
            return Some(MenuAction::SetLanguage(Language::SimplifiedChinese));
        }
        if layout.god_mode_button.contains(cursor) {
            return Some(MenuAction::ToggleGodMode);
        }
        if layout.resume_button.contains(cursor) {
            return Some(MenuAction::Resume);
        }
        if layout.reset_button.contains(cursor) {
            return Some(MenuAction::ResetScene);
        }
        if layout.quit_button.contains(cursor) {
            return Some(MenuAction::Quit);
        }
        None
    }
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    SetResolution(usize),
    SetLanguage(Language),
    ToggleGodMode,
    CycleSensitivity,
    CycleWeather,
    /// Load the scene at this index of `PauseMenu::scene_names`.
    LoadScene(usize),
    Resume,
    ResetScene,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UiRect {
    pub fn contains(&self, point: Vec2) -> bool {
        point.is_finite()
            && point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolutionButton {
    pub rect: UiRect,
    pub index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneButton {
    pub rect: UiRect,
    pub index: usize,
}

/// Pixel-space menu geometry shared by rendering and pointer hit testing.
///
/// The shell is capped on ultrawide displays so the two related columns stay
/// visually connected. At 720p the same geometry scales down to a compact but
/// still comfortably clickable layout.
#[derive(Debug, Clone)]
pub struct PauseMenuLayout {
    pub shell: UiRect,
    pub header: UiRect,
    pub panel: UiRect,
    pub key_panel: UiRect,
    pub footer: UiRect,
    pub resolution_buttons: Vec<ResolutionButton>,
    pub language_button: UiRect,
    pub god_mode_button: UiRect,
    pub sensitivity_button: UiRect,
    pub weather_button: UiRect,
    pub scene_buttons: Vec<SceneButton>,
    pub resume_button: UiRect,
    pub reset_button: UiRect,
    pub quit_button: UiRect,
    pub status_card: UiRect,
    pub movement_card: UiRect,
    pub interaction_card: UiRect,
    pub simulation_card: UiRect,
    pub scale: f32,
    pub compact: bool,
}

impl PauseMenuLayout {
    pub fn new(width: f32, height: f32) -> Self {
        Self::new_with_scenes(width, height, 0)
    }

    pub fn new_with_scenes(width: f32, height: f32, scene_count: usize) -> Self {
        let width = finite_dimension(width);
        let height = finite_dimension(height);
        let scale = (height / 1080.0).min(width / 1600.0).clamp(0.68, 1.22);
        let margin = (32.0 * scale)
            .clamp(16.0, 48.0)
            .min(width * 0.04)
            .min(height * 0.04);
        let available_w = (width - margin * 2.0).max(1.0);
        let available_h = (height - margin * 2.0).max(1.0);
        let shell_w = available_w.min(1780.0 * scale.max(0.96));
        let shell_h = available_h.min(960.0 * scale.max(0.88));
        let shell = UiRect {
            x: (width - shell_w) * 0.5,
            y: (height - shell_h) * 0.5,
            w: shell_w,
            h: shell_h,
        };

        let header_h = (76.0 * scale).clamp(52.0, 94.0).min(shell.h * 0.16);
        let footer_h = (38.0 * scale).clamp(28.0, 48.0).min(shell.h * 0.10);
        let header = UiRect {
            x: shell.x,
            y: shell.y,
            w: shell.w,
            h: header_h,
        };
        let footer = UiRect {
            x: shell.x,
            y: shell.bottom() - footer_h,
            w: shell.w,
            h: footer_h,
        };

        let body_pad = (18.0 * scale).clamp(12.0, 22.0);
        let column_gap = (18.0 * scale).clamp(12.0, 22.0);
        let body_x = shell.x + body_pad;
        let body_y = header.bottom() + body_pad * 0.62;
        let body_w = (shell.w - body_pad * 2.0).max(1.0);
        let body_h = (footer.y - body_y - body_pad * 0.62).max(1.0);
        let settings_ratio = if shell.w < 1450.0 { 0.55 } else { 0.53 };
        let panel_w = ((body_w - column_gap) * settings_ratio).max(1.0);
        let key_w = (body_w - column_gap - panel_w).max(1.0);
        let panel = UiRect {
            x: body_x,
            y: body_y,
            w: panel_w,
            h: body_h,
        };
        let key_panel = UiRect {
            x: panel.right() + column_gap,
            y: body_y,
            w: key_w,
            h: body_h,
        };

        let content_pad = (24.0 * scale).clamp(16.0, 30.0);
        let button_gap = (12.0 * scale).clamp(8.0, 15.0);
        let button_h = (46.0 * scale).clamp(32.0, 56.0);
        let button_w = ((panel.w - content_pad * 2.0 - button_gap) * 0.5).max(1.0);
        let button_start_y = panel.y + (72.0 * scale).clamp(48.0, 88.0);
        let mut resolution_buttons = Vec::with_capacity(RESOLUTION_OPTIONS.len());
        for index in 0..RESOLUTION_OPTIONS.len() {
            let col = index % 2;
            let row = index / 2;
            resolution_buttons.push(ResolutionButton {
                rect: UiRect {
                    x: panel.x + content_pad + col as f32 * (button_w + button_gap),
                    y: button_start_y + row as f32 * (button_h + button_gap),
                    w: button_w,
                    h: button_h,
                },
                index,
            });
        }
        let resolution_rows = RESOLUTION_OPTIONS.len().div_ceil(2) as f32;
        let resolution_bottom = button_start_y
            + resolution_rows * button_h
            + (resolution_rows - 1.0).max(0.0) * button_gap;
        let full_button_w = panel.w - content_pad * 2.0;
        let language_button = UiRect {
            x: panel.x + content_pad,
            y: resolution_bottom + (52.0 * scale).clamp(35.0, 64.0),
            w: full_button_w,
            h: button_h,
        };
        let god_mode_button = UiRect {
            x: panel.x + content_pad,
            y: language_button.bottom() + (52.0 * scale).clamp(35.0, 64.0),
            w: full_button_w,
            h: button_h,
        };
        let sensitivity_button = UiRect {
            x: panel.x + content_pad,
            y: god_mode_button.bottom() + (52.0 * scale).clamp(35.0, 64.0),
            w: full_button_w,
            h: button_h,
        };
        let weather_button = UiRect {
            x: panel.x + content_pad,
            y: sensitivity_button.bottom() + (52.0 * scale).clamp(35.0, 64.0),
            w: full_button_w,
            h: button_h,
        };
        let status_y = weather_button.bottom() + (22.0 * scale).clamp(15.0, 27.0);
        let desired_status_h = (96.0 * scale).clamp(64.0, 116.0);
        let status_card = UiRect {
            x: panel.x + content_pad,
            y: status_y,
            w: full_button_w,
            h: desired_status_h.min((panel.bottom() - content_pad - status_y).max(1.0)),
        };

        let key_content_x = key_panel.x + content_pad;
        let key_content_w = (key_panel.w - content_pad * 2.0).max(1.0);
        let key_top_y = key_panel.y + (72.0 * scale).clamp(48.0, 88.0);
        let card_gap = (12.0 * scale).clamp(8.0, 15.0);

        // Scene selector: one row of equal-width buttons at the top of the
        // key panel; session controls (resume/reset/quit) right below.
        let scene_h = if scene_count > 0 { button_h } else { 0.0 };
        let mut scene_buttons = Vec::with_capacity(scene_count);
        if scene_count > 0 {
            let gap = (10.0 * scale).clamp(6.0, 13.0);
            let per_w = ((key_content_w - gap * (scene_count.saturating_sub(1) as f32))
                / scene_count as f32)
                .max(40.0);
            for index in 0..scene_count {
                scene_buttons.push(SceneButton {
                    rect: UiRect {
                        x: key_content_x + index as f32 * (per_w + gap),
                        y: key_top_y,
                        w: per_w,
                        h: scene_h,
                    },
                    index,
                });
            }
        }
        let session_y = key_top_y + scene_h + if scene_count > 0 { card_gap } else { 0.0 };
        let session_gap = (10.0 * scale).clamp(6.0, 13.0);
        let session_w = ((key_content_w - session_gap * 2.0) / 3.0).max(40.0);
        let resume_button = UiRect {
            x: key_content_x,
            y: session_y,
            w: session_w,
            h: button_h,
        };
        let reset_button = UiRect {
            x: key_content_x + session_w + session_gap,
            y: session_y,
            w: session_w,
            h: button_h,
        };
        let quit_button = UiRect {
            x: key_content_x + (session_w + session_gap) * 2.0,
            y: session_y,
            w: session_w,
            h: button_h,
        };

        let key_start_y = session_y + button_h + card_gap;
        let cards_bottom = key_panel.bottom() - content_pad;
        let cards_h = (cards_bottom - key_start_y - card_gap * 2.0).max(3.0);
        let movement_h = cards_h * 0.28;
        let interaction_h = cards_h * 0.38;
        let movement_card = UiRect {
            x: key_content_x,
            y: key_start_y,
            w: key_content_w,
            h: movement_h,
        };
        let interaction_card = UiRect {
            x: key_content_x,
            y: movement_card.bottom() + card_gap,
            w: key_content_w,
            h: interaction_h,
        };
        let simulation_card = UiRect {
            x: key_content_x,
            y: interaction_card.bottom() + card_gap,
            w: key_content_w,
            h: (cards_bottom - interaction_card.bottom() - card_gap).max(1.0),
        };

        Self {
            shell,
            header,
            panel,
            key_panel,
            footer,
            resolution_buttons,
            language_button,
            god_mode_button,
            sensitivity_button,
            weather_button,
            scene_buttons,
            resume_button,
            reset_button,
            quit_button,
            status_card,
            movement_card,
            interaction_card,
            simulation_card,
            scale,
            compact: scale < 0.86 || key_panel.w < 560.0,
        }
    }
}

fn finite_dimension(value: f32) -> f32 {
    if value.is_finite() {
        value.max(1.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(rect: UiRect) -> Vec2 {
        Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5)
    }

    fn assert_inside(rect: UiRect, width: f32, height: f32) {
        assert!(rect.x >= -0.01, "left edge escaped: {rect:?}");
        assert!(rect.y >= -0.01, "top edge escaped: {rect:?}");
        assert!(rect.right() <= width + 0.01, "right edge escaped: {rect:?}");
        assert!(
            rect.bottom() <= height + 0.01,
            "bottom edge escaped: {rect:?}"
        );
        assert!(rect.w > 0.0 && rect.h > 0.0, "empty rectangle: {rect:?}");
    }

    #[test]
    fn layout_stays_inside_supported_viewports() {
        for (width, height) in [
            (1280.0, 720.0),
            (1600.0, 900.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3440.0, 1440.0),
        ] {
            let layout = PauseMenuLayout::new(width, height);
            for rect in [
                layout.shell,
                layout.header,
                layout.panel,
                layout.key_panel,
                layout.footer,
                layout.language_button,
                layout.god_mode_button,
                layout.status_card,
                layout.movement_card,
                layout.interaction_card,
                layout.simulation_card,
            ] {
                assert_inside(rect, width, height);
            }
            for button in layout.resolution_buttons {
                assert_inside(button.rect, width, height);
                assert!(button.rect.w >= 120.0);
                assert!(button.rect.h >= 32.0);
            }
        }
    }

    #[test]
    fn every_clickable_control_maps_to_the_expected_action() {
        let menu = PauseMenu::new();
        let layout = PauseMenuLayout::new(1280.0, 720.0);
        for button in &layout.resolution_buttons {
            assert_eq!(
                menu.hit_test(1280.0, 720.0, center(button.rect)),
                Some(MenuAction::SetResolution(button.index))
            );
        }
        assert_eq!(
            menu.hit_test(1280.0, 720.0, center(layout.language_button)),
            Some(MenuAction::SetLanguage(Language::SimplifiedChinese))
        );
        assert_eq!(
            menu.hit_test(1280.0, 720.0, center(layout.god_mode_button)),
            Some(MenuAction::ToggleGodMode)
        );
        assert_eq!(menu.hit_test(1280.0, 720.0, Vec2::ZERO), None);
    }

    #[test]
    fn ultrawide_layout_remains_grouped_and_centered() {
        let layout = PauseMenuLayout::new(3440.0, 1440.0);
        assert!(layout.shell.w < 2300.0);
        assert!((layout.shell.x + layout.shell.w * 0.5 - 1720.0).abs() < 0.1);
        assert!(layout.panel.right() < layout.key_panel.x);
    }

    #[test]
    fn rectangle_rejects_invalid_pointer_coordinates() {
        let rect = UiRect {
            x: 10.0,
            y: 10.0,
            w: 100.0,
            h: 50.0,
        };
        assert!(rect.contains(Vec2::new(10.0, 10.0)));
        assert!(rect.contains(Vec2::new(110.0, 60.0)));
        assert!(!rect.contains(Vec2::new(f32::NAN, 20.0)));
        assert!(!rect.contains(Vec2::new(111.0, 20.0)));
    }
}
