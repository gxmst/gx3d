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
    pub aim_blend: f32,
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
            aim_blend: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Smooth recoil recovery
        self.recoil_offset = self.recoil_offset.lerp(Vec3::ZERO, dt * 10.0);
        self.recoil_rotation = self.recoil_rotation.slerp(Quat::IDENTITY, dt * 10.0);
    }

    pub fn update_aim(&mut self, aiming: bool, dt: f32) {
        let target = if aiming { 1.0 } else { 0.0 };
        self.aim_blend += (target - self.aim_blend) * (dt * 12.0).clamp(0.0, 1.0);
    }

    pub fn apply_recoil(&mut self, impulse: Vec3) {
        self.recoil_offset += impulse;
        // Add some rotation for visual effect
        let pitch = Quat::from_rotation_x(impulse.y * 2.0);
        let yaw = Quat::from_rotation_y(impulse.x * 0.5);
        self.recoil_rotation = pitch * yaw * self.recoil_rotation;
    }

    pub fn world_matrix(&self, camera: &Camera) -> Mat4 {
        // Calculate weapon position in world space relative to camera.
        // Offsets are expressed in view space, where -Z is forward (matching
        // `camera.forward()`), +X is right and +Y is up. So a negative
        // `position.z` places the weapon *in front* of the camera; hence the
        // forward term subtracts `position.z` rather than adding it.
        let camera_forward = camera.forward();
        let camera_right = camera.right();
        let camera_up = camera.up();

        // Weapon offset from camera
        let hip = self.position;
        let ads = Vec3::new(0.0, -0.19, -0.72);
        let view_position = hip.lerp(ads, self.aim_blend);
        let mut offset = camera_right * view_position.x + camera_up * view_position.y
            - camera_forward * view_position.z;

        // Add recoil offset
        offset += camera_right * self.recoil_offset.x + camera_up * self.recoil_offset.y
            - camera_forward * self.recoil_offset.z;

        let position = camera.position + offset;

        // Weapon rotation = camera rotation * local rotation * recoil rotation
        let rotation = camera.rotation * self.rotation * self.recoil_rotation;

        Mat4::from_scale_rotation_translation(self.scale, rotation, position)
    }

    pub fn muzzle_world_matrix(&self, camera: &Camera, flash_scale: f32) -> Mat4 {
        // Same view-space convention as `world_matrix`: -Z is forward, so the
        // forward term subtracts `muzzle_offset.z` to sit in front of the camera.
        let hip = self.position;
        let ads = Vec3::new(0.0, -0.19, -0.72);
        let muzzle_offset = hip.lerp(ads, self.aim_blend)
            + Vec3::new(0.0, 0.06, -0.72)
            + self.recoil_offset * Vec3::new(1.0, 1.0, 0.4);
        let position =
            camera.position + camera.right() * muzzle_offset.x + camera.up() * muzzle_offset.y
                - camera.forward() * muzzle_offset.z;
        Mat4::from_scale_rotation_translation(Vec3::splat(flash_scale), camera.rotation, position)
    }
}
