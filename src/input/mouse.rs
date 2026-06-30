use super::InputState;
use glam::Vec2;
use winit::event::MouseButton;

pub struct MouseInput;

impl MouseInput {
    pub fn get_look_delta(input: &InputState, sensitivity: f32, invert_y: bool) -> Vec2 {
        let mut delta = input.mouse_delta * sensitivity;
        if invert_y {
            delta.y = -delta.y;
        }
        delta
    }

    pub fn is_shooting(input: &InputState) -> bool {
        input.is_mouse_pressed(MouseButton::Left)
    }

    pub fn is_aiming(input: &InputState) -> bool {
        input.is_mouse_pressed(MouseButton::Right)
    }

    pub fn is_just_shooting(input: &InputState) -> bool {
        input.is_mouse_just_pressed(MouseButton::Left)
    }
}
