pub mod keyboard;
pub mod mouse;

pub use keyboard::*;
pub use mouse::*;

use glam::Vec2;
use std::collections::HashMap;
use winit::event::{ElementState, MouseButton};
use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
    Held,
}

impl ButtonState {
    pub fn is_pressed(&self) -> bool {
        matches!(self, Self::Pressed)
    }

    pub fn is_released(&self) -> bool {
        matches!(self, Self::Released)
    }

    pub fn is_held(&self) -> bool {
        matches!(self, Self::Held)
    }
}

#[derive(Debug, Clone)]
pub struct InputState {
    pub keys: HashMap<KeyCode, ButtonState>,
    pub mouse_buttons: HashMap<MouseButton, ButtonState>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub mouse_scroll: f32,
    pub window_focused: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            mouse_buttons: HashMap::new(),
            mouse_position: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            mouse_scroll: 0.0,
            window_focused: true,
        }
    }

    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys
            .get(&key)
            .is_some_and(|s| s.is_pressed() || s.is_held())
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys.get(&key).is_some_and(|s| s.is_pressed())
    }

    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .is_some_and(|s| s.is_pressed() || s.is_held())
    }

    pub fn is_mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .is_some_and(|s| s.is_pressed())
    }

    pub fn update(&mut self) {
        for state in self.keys.values_mut() {
            if *state == ButtonState::Pressed {
                *state = ButtonState::Held;
            }
        }
        for state in self.mouse_buttons.values_mut() {
            if *state == ButtonState::Pressed {
                *state = ButtonState::Held;
            }
        }
        self.mouse_delta = Vec2::ZERO;
        self.mouse_scroll = 0.0;
    }

    pub fn process_key(&mut self, key: KeyCode, state: ElementState) {
        let button_state = match state {
            ElementState::Pressed => ButtonState::Pressed,
            ElementState::Released => ButtonState::Released,
        };
        self.keys.insert(key, button_state);
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        let button_state = match state {
            ElementState::Pressed => ButtonState::Pressed,
            ElementState::Released => ButtonState::Released,
        };
        self.mouse_buttons.insert(button, button_state);
    }

    pub fn process_mouse_motion(&mut self, delta: Vec2) {
        self.mouse_delta += delta;
    }

    pub fn process_mouse_scroll(&mut self, delta: f32) {
        self.mouse_scroll += delta;
    }

    pub fn process_cursor_position(&mut self, position: Vec2) {
        self.mouse_position = position;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MouseSettings {
    pub sensitivity: f32,
    pub invert_y: bool,
}

impl MouseSettings {
    pub fn new() -> Self {
        Self {
            sensitivity: 0.002,
            invert_y: false,
        }
    }

    pub fn apply(&self, delta: Vec2) -> Vec2 {
        let mut adjusted = delta * self.sensitivity;
        if self.invert_y {
            adjusted.y = -adjusted.y;
        }
        adjusted
    }
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self::new()
    }
}
