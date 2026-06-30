use super::InputState;
use winit::keyboard::KeyCode;

pub struct KeyboardInput;

impl KeyboardInput {
    pub fn get_movement_vector(input: &InputState) -> glam::Vec3 {
        let mut movement = glam::Vec3::ZERO;

        if input.is_key_pressed(KeyCode::KeyW) {
            movement.z -= 1.0;
        }
        if input.is_key_pressed(KeyCode::KeyS) {
            movement.z += 1.0;
        }
        if input.is_key_pressed(KeyCode::KeyA) {
            movement.x -= 1.0;
        }
        if input.is_key_pressed(KeyCode::KeyD) {
            movement.x += 1.0;
        }
        if input.is_key_pressed(KeyCode::Space) {
            movement.y += 1.0;
        }
        if input.is_key_pressed(KeyCode::ControlLeft) {
            movement.y -= 1.0;
        }

        if movement.length_squared() > 0.0 {
            movement.normalize()
        } else {
            movement
        }
    }

    pub fn is_jump_pressed(input: &InputState) -> bool {
        input.is_key_just_pressed(KeyCode::Space)
    }

    pub fn is_crouch_pressed(input: &InputState) -> bool {
        input.is_key_pressed(KeyCode::ControlLeft)
    }

    pub fn is_sprint_pressed(input: &InputState) -> bool {
        input.is_key_pressed(KeyCode::ShiftLeft)
    }

    pub fn is_reload_pressed(input: &InputState) -> bool {
        input.is_key_just_pressed(KeyCode::KeyR)
    }

    pub fn is_interact_pressed(input: &InputState) -> bool {
        input.is_key_just_pressed(KeyCode::KeyE)
    }
}
