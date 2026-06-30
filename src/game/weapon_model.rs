use crate::asset::{Handle, Material, Mesh};
use crate::renderer::Camera;
use glam::{Mat4, Quat, Vec3};

pub struct WeaponModel {
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub recoil_offset: Vec3,
    pub recoil_rotation: Quat,
}

impl WeaponModel {
    pub fn new(mesh: Handle<Mesh>, material: Handle<Material>) -> Self {
        Self {
            mesh,
            material,
            position: Vec3::new(0.15, -0.12, -0.25),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(0.02, 0.02, 0.12),
            recoil_offset: Vec3::ZERO,
            recoil_rotation: Quat::IDENTITY,
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Smooth recoil recovery
        self.recoil_offset = self.recoil_offset.lerp(Vec3::ZERO, dt * 10.0);
        self.recoil_rotation = self.recoil_rotation.slerp(Quat::IDENTITY, dt * 10.0);
    }

    pub fn apply_recoil(&mut self, impulse: Vec3) {
        self.recoil_offset += impulse;
        // Add some rotation for visual effect
        let pitch = Quat::from_rotation_x(impulse.y * 2.0);
        let yaw = Quat::from_rotation_y(impulse.x * 0.5);
        self.recoil_rotation = pitch * yaw * self.recoil_rotation;
    }

    pub fn world_matrix(&self, camera: &Camera) -> Mat4 {
        // Calculate weapon position in world space relative to camera
        let camera_forward = camera.forward();
        let camera_right = camera.right();
        let camera_up = camera.up();

        // Weapon offset from camera
        let mut offset = camera_right * self.position.x
            + camera_up * self.position.y
            + camera_forward * self.position.z;

        // Add recoil offset
        offset += camera_right * self.recoil_offset.x
            + camera_up * self.recoil_offset.y
            + camera_forward * self.recoil_offset.z;

        let position = camera.position + offset;

        // Weapon rotation = camera rotation * local rotation * recoil rotation
        let rotation = camera.rotation * self.rotation * self.recoil_rotation;

        Mat4::from_scale_rotation_translation(self.scale, rotation, position)
    }

    pub fn muzzle_world_matrix(&self, camera: &Camera, flash_scale: f32) -> Mat4 {
        let muzzle_offset = self.position
            + Vec3::new(0.0, 0.06, -0.72)
            + self.recoil_offset * Vec3::new(1.0, 1.0, 0.4);
        let position = camera.position
            + camera.right() * muzzle_offset.x
            + camera.up() * muzzle_offset.y
            + camera.forward() * muzzle_offset.z;
        Mat4::from_scale_rotation_translation(Vec3::splat(flash_scale), camera.rotation, position)
    }
}
