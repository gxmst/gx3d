use crate::asset::{Handle, Material, Mesh};
use crate::renderer::Camera;
use glam::{Mat4, Quat, Vec3};

/// View-space weapon anchor while aiming down sights. `world_matrix` (weapon
/// pose) and `muzzle_world_matrix` (flash pose) must agree on this.
const ADS_OFFSET: Vec3 = Vec3::new(0.0, -0.19, -0.72);

pub struct WeaponModel {
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub recoil_offset: Vec3,
    pub recoil_rotation: Quat,
    pub aim_blend: f32,
    /// 0..1 reload animation progress (0 = idle). Drives a dip-and-tilt pose.
    pub reload_progress: f32,
    /// Seconds the full reload animation lasts (synced to the weapon's
    /// reload duration when the reload starts).
    pub reload_length: f32,
    /// Remaining seconds of the draw (weapon switch) animation: the new gun
    /// rises from below with a slight roll.
    pub draw_timer: f32,
}

/// Seconds the draw animation lasts after switching weapons.
pub const DRAW_SECONDS: f32 = 0.35;

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
            reload_progress: 0.0,
            reload_length: 2.0,
            draw_timer: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Smooth recoil recovery
        self.recoil_offset = self.recoil_offset.lerp(Vec3::ZERO, dt * 10.0);
        self.recoil_rotation = self.recoil_rotation.slerp(Quat::IDENTITY, dt * 10.0);
        if self.draw_timer > 0.0 {
            self.draw_timer = (self.draw_timer - dt).max(0.0);
        }
    }

    /// Advance the reload pose. `remaining` is the weapon's reload timer and
    /// `duration` its full reload length; progress runs 0→1 over the reload
    /// and snaps back to 0 when done.
    pub fn update_reload(&mut self, reloading: bool, remaining: f32, duration: f32) {
        self.reload_length = duration.max(0.1);
        if reloading {
            self.reload_progress = (1.0 - remaining / self.reload_length).clamp(0.0, 1.0);
        } else {
            self.reload_progress = 0.0;
        }
    }

    /// Begin the draw (switch) animation.
    pub fn start_draw_anim(&mut self) {
        self.draw_timer = DRAW_SECONDS;
    }

    /// Composite animation pose offset/rotation in view space.
    ///
    /// Reload: the gun dips down-right while rolling outward, hits the lowest
    /// point mid-reload (magazine swap), then returns. A sine hump keeps both
    /// ends of the motion smooth.
    /// Draw: the gun rises from below with a forward pitch that settles.
    fn anim_pose(&self) -> (Vec3, Quat) {
        let mut offset = Vec3::ZERO;
        let mut rotation = Quat::IDENTITY;
        if self.reload_progress > 0.0 {
            let hump = (self.reload_progress * std::f32::consts::PI).sin();
            offset += Vec3::new(0.05, -0.16, 0.05) * hump;
            rotation = Quat::from_rotation_z(-0.7 * hump) * Quat::from_rotation_x(0.35 * hump);
        }
        if self.draw_timer > 0.0 {
            let t = (self.draw_timer / DRAW_SECONDS).clamp(0.0, 1.0);
            // t runs 1→0; ease-out so the rise decelerates into place.
            let ease = t * t;
            offset += Vec3::new(0.0, -0.28, 0.08) * ease;
            rotation = Quat::from_rotation_x(0.8 * ease) * rotation;
        }
        (offset, rotation)
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
        let (anim_offset, anim_rotation) = self.anim_pose();
        // Reload/draw poses read badly while aiming down sights; hip pose wins.
        let view_position = hip.lerp(ADS_OFFSET, self.aim_blend) + anim_offset;
        let mut offset = camera_right * view_position.x + camera_up * view_position.y
            - camera_forward * view_position.z;

        // Add recoil offset
        offset += camera_right * self.recoil_offset.x + camera_up * self.recoil_offset.y
            - camera_forward * self.recoil_offset.z;

        let position = camera.position + offset;

        // Weapon rotation = camera * local * animation * recoil
        let rotation = camera.rotation * self.rotation * anim_rotation * self.recoil_rotation;

        Mat4::from_scale_rotation_translation(self.scale, rotation, position)
    }

    pub fn muzzle_world_matrix(&self, camera: &Camera, flash_scale: f32) -> Mat4 {
        // Same view-space convention as `world_matrix`: -Z is forward, so the
        // forward term subtracts `muzzle_offset.z` to sit in front of the camera.
        let hip = self.position;
        let muzzle_offset = hip.lerp(ADS_OFFSET, self.aim_blend)
            + Vec3::new(0.0, 0.06, -0.72)
            + self.recoil_offset * Vec3::new(1.0, 1.0, 0.4);
        let position =
            camera.position + camera.right() * muzzle_offset.x + camera.up() * muzzle_offset.y
                - camera.forward() * muzzle_offset.z;
        Mat4::from_scale_rotation_translation(Vec3::splat(flash_scale), camera.rotation, position)
    }
}
