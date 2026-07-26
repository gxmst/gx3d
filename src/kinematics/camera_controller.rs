use crate::input::InputState;
use crate::kinematics::RecoilSystem;
use crate::renderer::Camera;
use glam::{Quat, Vec2};

#[derive(Debug, Clone)]
pub struct CameraController {
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    pub pitch: f32,
    pub yaw: f32,
    pub recoil_system: RecoilSystem,
}

impl CameraController {
    pub fn new(move_speed: f32, mouse_sensitivity: f32) -> Self {
        Self {
            move_speed,
            sprint_multiplier: 1.5,
            mouse_sensitivity,
            invert_y: false,
            pitch: 0.0,
            yaw: 0.0,
            recoil_system: RecoilSystem::default(),
        }
    }

    pub fn update_camera_rotation(&mut self, camera: &mut Camera, input: &InputState, dt: f32) {
        // Mouse look
        let mouse_delta = input.mouse_delta * self.mouse_sensitivity;
        self.yaw -= mouse_delta.x;
        self.pitch -= if self.invert_y {
            -mouse_delta.y
        } else {
            mouse_delta.y
        };
        self.pitch = self
            .pitch
            .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

        // Update recoil
        self.recoil_system.update(dt);
        let recoil_offset = self.recoil_system.offset();

        // Apply rotation
        let yaw_rotation = Quat::from_rotation_y(self.yaw);
        let pitch_rotation = Quat::from_rotation_x(self.pitch + recoil_offset.y);
        camera.rotation = yaw_rotation * pitch_rotation;
    }

    pub fn apply_recoil(&mut self, impulse: Vec2) {
        self.recoil_system.apply_impulse(impulse);
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new(8.0, 0.003)
    }
}
