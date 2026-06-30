use crate::input::InputState;
use crate::kinematics::CameraController;
use crate::renderer::Camera;

pub struct Player {
    pub camera_controller: CameraController,
    pub height: f32,
    pub is_grounded: bool,
}

impl Player {
    pub fn new() -> Self {
        Self {
            camera_controller: CameraController::default(),
            height: 1.6,
            is_grounded: true,
        }
    }

    pub fn update(&mut self, camera: &mut Camera, input: &InputState, dt: f32) {
        self.camera_controller.update(camera, input, dt);
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
