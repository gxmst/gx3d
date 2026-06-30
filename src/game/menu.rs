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

pub const RESOLUTION_OPTIONS: [ResolutionOption; 4] = [
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
];

#[derive(Debug, Clone)]
pub struct PauseMenu {
    pub open: bool,
    pub language: Language,
    pub selected_resolution: usize,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            language: Language::SimplifiedChinese,
            selected_resolution: 2,
        }
    }

    pub fn hit_test(&self, width: f32, height: f32, cursor: Vec2) -> Option<MenuAction> {
        let layout = PauseMenuLayout::new(width, height);
        for button in layout.resolution_buttons {
            if button.rect.contains(cursor) {
                return Some(MenuAction::SetResolution(button.index));
            }
        }
        if layout.language_button.contains(cursor) {
            return Some(MenuAction::SetLanguage(Language::SimplifiedChinese));
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
}

#[derive(Debug, Clone, Copy)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UiRect {
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.w
            && point.y >= self.y
            && point.y <= self.y + self.h
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolutionButton {
    pub rect: UiRect,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct PauseMenuLayout {
    pub panel: UiRect,
    pub key_panel: UiRect,
    pub resolution_buttons: Vec<ResolutionButton>,
    pub language_button: UiRect,
}

impl PauseMenuLayout {
    pub fn new(width: f32, height: f32) -> Self {
        let gap = 24.0;
        let panel_w = (width * 0.42).clamp(520.0, 760.0);
        let panel_h = (height * 0.66).clamp(500.0, 720.0);
        let panel_x = (width * 0.08).max(64.0);
        let panel_y = ((height - panel_h) * 0.5).max(40.0);
        let key_x = panel_x + panel_w + gap;
        let key_w = (width - key_x - panel_x).max(360.0);

        let mut resolution_buttons = Vec::new();
        let button_w = 210.0;
        let button_h = 44.0;
        let start_x = panel_x + 42.0;
        let start_y = panel_y + 160.0;
        for index in 0..RESOLUTION_OPTIONS.len() {
            let col = index % 2;
            let row = index / 2;
            resolution_buttons.push(ResolutionButton {
                rect: UiRect {
                    x: start_x + col as f32 * (button_w + 18.0),
                    y: start_y + row as f32 * (button_h + 16.0),
                    w: button_w,
                    h: button_h,
                },
                index,
            });
        }

        Self {
            panel: UiRect {
                x: panel_x,
                y: panel_y,
                w: panel_w,
                h: panel_h,
            },
            key_panel: UiRect {
                x: key_x,
                y: panel_y,
                w: key_w,
                h: panel_h,
            },
            resolution_buttons,
            language_button: UiRect {
                x: start_x,
                y: start_y + 2.0 * (button_h + 16.0) + 70.0,
                w: button_w,
                h: button_h,
            },
        }
    }
}
