use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
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

    pub fn translate(&mut self, offset: Vec3) {
        self.position += offset;
    }

    pub fn rotate(&mut self, rotation: Quat) {
        self.rotation = rotation * self.rotation;
    }

    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let forward = (target - self.position).normalize();
        let right = forward.cross(up).normalize();
        let corrected_up = right.cross(forward);
        let rot_matrix = Mat4::from_cols(
            glam::Vec4::new(right.x, right.y, right.z, 0.0),
            glam::Vec4::new(corrected_up.x, corrected_up.y, corrected_up.z, 0.0),
            glam::Vec4::new(-forward.x, -forward.y, -forward.z, 0.0),
            glam::Vec4::W,
        );
        self.rotation = Quat::from_mat4(&rot_matrix);
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct Frozen;

impl Default for Velocity {
    fn default() -> Self {
        Self {
            linear: Vec3::ZERO,
            angular: Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn ratio(&self) -> f32 {
        self.current / self.max
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Name(pub &'static str);

impl Default for Name {
    fn default() -> Self {
        Self("Unnamed")
    }
}
