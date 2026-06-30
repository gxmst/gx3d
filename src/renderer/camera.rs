use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(position: Vec3, fov: f32, aspect: f32) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            fov,
            aspect,
            near: 0.1,
            far: 1000.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let forward = self.rotation * -Vec3::Z;
        let up = self.rotation * Vec3::Y;
        Mat4::look_to_rh(self.position, forward, up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far)
    }

    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * -Vec3::Z
    }

    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    pub fn look_at(&mut self, target: Vec3) {
        let forward = (target - self.position).normalize();
        self.rotation = Quat::from_rotation_arc(-Vec3::Z, forward);
    }

    pub fn rotate(&mut self, yaw: f32, pitch: f32) {
        let yaw_rotation = Quat::from_rotation_y(yaw);
        let pitch_rotation = Quat::from_rotation_x(pitch);
        self.rotation = yaw_rotation * self.rotation * pitch_rotation;
    }

    pub fn set_aspect(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(Vec3::new(0.0, 1.6, 5.0), 70.0, 16.0 / 9.0)
    }
}
