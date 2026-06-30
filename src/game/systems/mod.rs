use crate::core::{Schedule, Stage};

pub mod enemy;
pub mod input;
pub mod menu;
pub mod physics_sync;
pub mod player;
pub mod render;
pub mod sandbox;
pub mod setup;
pub mod weapon;

/// Per-frame state: whether the mouse is locked for FPS look.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseLocked(pub bool);

/// Remaining time for the muzzle flash visual feedback.
#[derive(Debug, Clone, Copy, Default)]
pub struct MuzzleFlashTimer(pub f32);

/// Runtime settings menu state.
#[derive(Debug, Clone, Default)]
pub struct MenuState(pub crate::game::PauseMenu);

/// Handle to the player's dynamic rigid body in Rapier.
#[derive(Debug, Clone, Copy)]
pub struct PlayerBody(pub rapier3d::prelude::RigidBodyHandle);

/// Lights in the current scene, uploaded to GPU each frame.
#[derive(Debug, Clone, Default)]
pub struct SceneLights(pub Vec<crate::renderer::Light>);

/// Whether the one-key control hint is currently held open.
#[derive(Debug, Clone, Copy, Default)]
pub struct HelpOverlay(pub bool);

/// GPU texture views for asset textures, keyed by asset id.
#[derive(Debug, Clone, Default)]
pub struct TextureViews(pub std::collections::HashMap<u64, wgpu::TextureView>);

/// Enemies spawned into the world.
#[derive(Debug, Clone, Default)]
pub struct Enemies(pub Vec<hecs::Entity>);

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
    schedule.add_system(Stage::Update, player::input_system);
    schedule.add_system(Stage::Update, weapon::update_system);
    schedule.add_system(Stage::Update, sandbox::system);
    schedule.add_system(Stage::FixedUpdate, physics_sync::step_physics);
    schedule.add_system(Stage::LateUpdate, player::sync_system);
    schedule.add_system(Stage::LateUpdate, enemy::system);
    schedule.add_system(Stage::LateUpdate, enemy::feedback_system);
    schedule.add_system(Stage::Render, render::system);
    schedule.add_system(Stage::PostRender, input::system);
}
