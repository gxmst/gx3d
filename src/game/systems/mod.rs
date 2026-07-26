use crate::core::{Resources, Schedule, Stage};

pub mod bot_weapon;
pub mod buy_menu;
pub mod combat;
pub mod debug_panel;
pub mod effects;
pub mod enemy;
pub mod input;
pub mod interaction;
pub mod match_mode;
pub mod menu;
pub mod physics_debug;
pub mod physics_sync;
pub mod player;
pub mod ragdoll;
pub mod render;
pub mod sandbox;
pub mod setup;
pub mod time_control;
pub mod view_mode;
pub mod weapon;
pub mod weather;

/// Per-frame state: whether the mouse is locked for FPS look.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseLocked(pub bool);

/// Transient on-screen notification (scene load failures, respawns, ...).
#[derive(Debug, Clone, Default)]
pub struct Toast {
    pub message: String,
    pub remaining: f32,
}

impl Toast {
    pub fn show(&mut self, message: impl Into<String>, seconds: f32) {
        self.message = message.into();
        self.remaining = seconds.max(0.5);
    }

    pub fn visible(&self) -> Option<&str> {
        (self.remaining > 0.0 && !self.message.is_empty()).then_some(self.message.as_str())
    }
}

/// The player's combat state. Reset on scene load and on respawn.
#[derive(Debug, Clone, Copy)]
pub struct PlayerHealth {
    pub current: f32,
    pub max: f32,
    /// 0..1 red screen-flash intensity, decays each frame.
    pub hurt_flash: f32,
    /// Post-respawn grace seconds during which enemies hold fire.
    pub invulnerable: f32,
    pub deaths: u32,
}

impl Default for PlayerHealth {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
            hurt_flash: 0.0,
            invulnerable: 0.0,
            deaths: 0,
        }
    }
}

/// Seconds a muzzle flash stays visible. The render system divides the timer
/// by this same value for the flash scale curve.
pub const MUZZLE_FLASH_SECONDS: f32 = 0.06;

/// Remaining time for the muzzle flash visual feedback.
#[derive(Debug, Clone, Copy, Default)]
pub struct MuzzleFlashTimer(pub f32);

/// Whether the pause menu currently captures input. Gameplay systems
/// early-return while it is open.
pub(crate) fn menu_open(resources: &Resources) -> bool {
    resources
        .get::<MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
}

/// Cast a ray from the camera through the crosshair, ignoring the player's
/// own capsule.
pub(crate) fn crosshair_raycast(
    resources: &Resources,
    max_distance: f32,
) -> Option<crate::physics::RaycastHit> {
    let (origin, direction) = {
        let camera = resources.expect::<crate::renderer::Camera>();
        (camera.position, camera.forward())
    };
    let player_body = resources.expect::<PlayerBody>().0;
    let physics = resources.expect::<crate::physics::PhysicsWorld>();
    physics.cast_ray_excluding_body(
        &crate::physics::Ray::new(origin, direction, max_distance),
        player_body,
    )
}

/// Short HUD confirmation pulse after a raycast hits an entity or surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct HitMarkerTimer(pub f32);

/// Runtime settings menu state.
#[derive(Debug, Clone, Default)]
pub struct MenuState(pub crate::game::PauseMenu);

/// Handle to the player's dynamic rigid body in Rapier.
#[derive(Debug, Clone, Copy)]
pub struct PlayerBody(pub rapier3d::prelude::RigidBodyHandle);

/// Lights in the current scene, uploaded to GPU each frame.
#[derive(Debug, Clone, Default)]
pub struct SceneLights(pub Vec<crate::renderer::Light>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Fps,
    God,
}

#[derive(Debug, Clone, Copy)]
pub struct ViewModeState {
    pub mode: ViewMode,
    pub god_mode_enabled: bool,
    pub fly_speed: f32,
}

impl Default for ViewModeState {
    fn default() -> Self {
        Self {
            mode: ViewMode::Fps,
            god_mode_enabled: crate::game::menu::DEFAULT_GOD_MODE_ENABLED,
            fly_speed: 12.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicsDebugState {
    pub colliders: bool,
    pub velocities: bool,
    pub contacts: bool,
    /// F6: recent impulse arrows (position + direction + fading magnitude).
    pub impulses: bool,
}

/// Per-catalog-slot first-person weapon meshes, index-aligned with
/// `game::weapon::WEAPON_CATALOG`.
#[derive(Debug, Clone)]
pub struct WeaponMeshes(pub Vec<crate::asset::Handle<crate::asset::Mesh>>);

/// GPU-ready resources for weapon hit feedback spawned at runtime.
#[derive(Debug, Clone, Copy)]
pub struct WeaponFeedbackAssets {
    pub bullet_hole_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub bullet_hole_material: crate::asset::Handle<crate::asset::Material>,
    pub impact_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub impact_material: crate::asset::Handle<crate::asset::Material>,
    pub muzzle_flash_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub muzzle_flash_material: crate::asset::Handle<crate::asset::Material>,
}

/// Auto-despawns temporary visual effects.
#[derive(Debug, Clone, Copy)]
pub struct TimedEffect {
    pub remaining: f32,
}

/// Tracks an enemy's "hit flash": while active, the enemy renders with a bright
/// emissive material so a hit reads instantly. When the timer runs out the
/// original material is restored.
#[derive(Debug, Clone, Copy)]
pub struct HitFlash {
    pub remaining: f32,
    pub original_material: crate::asset::Handle<crate::asset::Material>,
}

/// The shared bright material swapped onto enemies while they flash from a hit.
/// Created during setup as engine plumbing (not scene content).
#[derive(Debug, Clone, Copy)]
pub struct EnemyFlashMaterial(pub crate::asset::Handle<crate::asset::Material>);

pub fn register_default_systems(schedule: &mut Schedule) {
    schedule.add_system(Stage::Update, menu::system);
    schedule.add_system(Stage::Update, time_control::system);
    schedule.add_system(Stage::Update, view_mode::system);
    schedule.add_system(Stage::Update, physics_debug::system);
    schedule.add_system(Stage::Update, interaction::system);
    schedule.add_system(Stage::Update, player::input_system);
    schedule.add_system(Stage::Update, buy_menu::update);
    schedule.add_system(Stage::Update, weapon::update_system);
    schedule.add_system(Stage::Update, sandbox::system);
    schedule.add_system(Stage::Update, ragdoll::update);
    schedule.add_system(Stage::Update, effects::update);
    schedule.add_system(Stage::Update, combat::update);
    schedule.add_system(Stage::Update, weather::update);
    // Effect forces must accumulate before the physics step consumes them.
    schedule.add_system(Stage::FixedUpdate, effects::fixed_update);
    schedule.add_system(Stage::FixedUpdate, player::fixed_update_system);
    schedule.add_system(Stage::FixedUpdate, enemy::system);
    // Combat runs after patrol movement so LOS checks use current positions.
    schedule.add_system(Stage::FixedUpdate, combat::fixed_update);
    schedule.add_system(Stage::FixedUpdate, match_mode::fixed_update);
    schedule.add_system(Stage::FixedUpdate, interaction::fixed_update);
    schedule.add_system(Stage::FixedUpdate, sandbox::fixed_update);
    schedule.add_system(Stage::FixedUpdate, physics_sync::step_physics);
    schedule.add_system(Stage::LateUpdate, player::sync_system);
    schedule.add_system(Stage::LateUpdate, enemy::feedback_system);
    // After movement/feedback: held rifles snap to final actor poses.
    schedule.add_system(Stage::LateUpdate, bot_weapon::update);
    schedule.add_system(Stage::Render, render::system);
    schedule.add_system(Stage::PostRender, input::system);
}
