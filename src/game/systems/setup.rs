use crate::asset::{AssetManager, Handle, Material};
use crate::core::{EngineWorld, Resources, Schedule, Stage};
use crate::game::scene::{spawn_scene, Scene};
use crate::game::{Player, Weapon, WeaponModel};
use crate::physics::{PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Camera, Renderer};
use glam::Vec3;
use std::collections::HashMap;

use super::{
    Enemies, EnemyFlashMaterial, MenuState, MouseLocked, MuzzleFlashTimer, PhysicsDebugState,
    PlayerBody, SceneLights, TextureViews, ViewModeState, WeaponFeedbackAssets,
};

/// The default scene, compiled into the binary as a fallback so the engine can
/// always start even if the on-disk scene file is missing or malformed.
const EMBEDDED_SCENE: &str = include_str!("../../../assets/scenes/dust2.json");

/// Path (relative to the working directory) of the scene loaded at startup.
/// Editing this file lets you change the level without recompiling.
const DEFAULT_SCENE_PATH: &str = "assets/scenes/dust2.json";

pub fn register(schedule: &mut Schedule) {
    schedule.add_system(Stage::Startup, setup_scene);
}

/// Load the startup scene: prefer the on-disk file (so edits take effect
/// without recompiling), and fall back to the embedded copy if it is missing
/// or fails to parse. Either way the engine starts with a valid scene.
fn load_scene() -> Scene {
    let scene_path = requested_scene_path();
    match std::fs::read_to_string(&scene_path) {
        Ok(text) => match Scene::from_json(&text) {
            Ok(scene) => {
                log::info!("Loaded scene from {}", scene_path.display());
                return scene;
            }
            Err(e) => log::error!(
                "Failed to parse {}: {e}. Falling back to embedded scene.",
                scene_path.display()
            ),
        },
        Err(e) => log::info!(
            "No scene file at {} ({e}). Using embedded scene.",
            scene_path.display()
        ),
    }
    // The embedded scene is authored alongside the code and is expected to
    // always parse; if it does not, that is a build-time bug worth surfacing.
    Scene::from_json(EMBEDDED_SCENE).expect("embedded scene must parse")
}

fn requested_scene_path() -> std::path::PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--scene" {
            if let Some(value) = args.next() {
                let path = std::path::PathBuf::from(&value);
                return if path.extension().is_some() || path.components().count() > 1 {
                    path
                } else {
                    std::path::Path::new("assets/scenes")
                        .join(value)
                        .with_extension("json")
                };
            }
        }
    }
    std::path::PathBuf::from(DEFAULT_SCENE_PATH)
}

fn setup_scene(world: &mut EngineWorld, resources: &Resources) {
    let scene = load_scene();

    let mut asset_manager = resources
        .remove::<AssetManager>()
        .expect("AssetManager missing");
    let mut physics_world = resources
        .remove::<PhysicsWorld>()
        .expect("PhysicsWorld missing");

    // Instantiate the data-driven scene: meshes, materials, level geometry,
    // lights, and enemies. Everything after this point is engine plumbing that
    // the scene only references by name.
    let spawned = spawn_scene(&scene, world, &mut physics_world, &mut asset_manager);

    // === Camera & player tuning (from the scene's player block) ===
    {
        let mut camera = resources.expect_mut::<Camera>();
        camera.position = Vec3::from_array(spawned.player.position);
    }
    {
        let mut player = resources.expect_mut::<Player>();
        player.camera_controller.move_speed = spawned.player.move_speed;
        player.camera_controller.mouse_sensitivity = spawned.player.mouse_sensitivity;
        player.height = spawned.player.height;
    }

    // === Player physics body (engine plumbing, not scene content) ===
    let player_rb = {
        let camera = resources.expect::<Camera>();
        let collider = PhysicsShape::Capsule {
            radius: 0.3,
            half_height: 0.5,
        };
        let (rb, _col) = physics_world.add_dynamic_body(
            camera.position,
            collider.to_rapier_collider_with_material(PhysicsMaterial {
                friction: 0.0,
                restitution: 0.0,
            }),
            1.0,
        );
        if let Some(rb_ref) = physics_world.rigid_body_set.get_mut(rb) {
            rb_ref.lock_rotations(true, true);
            rb_ref.set_dominance_group(10);
            rb_ref.set_linear_damping(0.08);
            rb_ref.enable_ccd(true);
            rb_ref.set_soft_ccd_prediction(0.25);
        }
        rb
    };
    resources.insert(PlayerBody(player_rb));
    resources.insert(MouseLocked(true));
    resources.insert(MenuState::default());
    resources.insert(MuzzleFlashTimer(0.0));
    resources.insert(ViewModeState::default());
    resources.insert(PhysicsDebugState::default());
    resources.insert(crate::game::InteractionFocus::default());

    // === Weapon (engine plumbing) ===
    // The rifle mesh comes from the scene; its material is engine-defined.
    let rifle_mesh = spawned.mesh("rifle").unwrap_or_default();
    let weapon_material = asset_manager
        .materials
        .insert(Material::metal([0.18, 0.2, 0.22]));
    let mut weapon_model = WeaponModel::new(rifle_mesh, weapon_material);
    weapon_model.position = Vec3::new(0.36, -0.30, -0.58);
    weapon_model.scale = Vec3::splat(0.9);
    resources.insert(Weapon::new("Rifle", 25.0, 10.0, 30));
    resources.insert(weapon_model);

    // === Hit-feedback assets (engine plumbing spawned at runtime) ===
    let bullet_hole_material = asset_manager.materials.insert(solid_material(
        "Bullet Impact Mark",
        [0.018, 0.014, 0.011],
        0.72,
        0.18,
    ));
    let muzzle_flash_material = asset_manager.materials.insert(Material {
        name: "Muzzle Flash".to_string(),
        albedo_factor: [1.0, 0.58, 0.14, 1.0],
        metallic: 0.0,
        roughness: 0.2,
        emissive_factor: [4.0, 1.8, 0.25],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    let cube_mesh = spawned.mesh("cube").unwrap_or_default();
    let sphere_mesh = spawned.mesh("sphere").unwrap_or_default();
    let cylinder_mesh = spawned.mesh("cylinder").unwrap_or(cube_mesh);
    resources.insert(WeaponFeedbackAssets {
        bullet_hole_mesh: cylinder_mesh,
        bullet_hole_material,
        impact_mesh: sphere_mesh,
        impact_material: muzzle_flash_material,
        muzzle_flash_mesh: sphere_mesh,
        muzzle_flash_material,
    });

    // Bright emissive material swapped onto enemies for a moment when hit. It is
    // created here (before the GPU upload below) so its bind group is cached and
    // the render system never misses it.
    let enemy_flash_material = asset_manager.materials.insert(Material {
        name: "Enemy Flash".to_string(),
        albedo_factor: [1.0, 1.0, 1.0, 1.0],
        metallic: 0.0,
        roughness: 0.4,
        emissive_factor: [6.0, 5.2, 5.0],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    resources.insert(EnemyFlashMaterial(enemy_flash_material));

    // === Physics sandbox shared handles (for runtime G/B/H spawning) ===
    let physics_sandbox = crate::physics::sandbox::PhysicsSandbox {
        cube_mesh,
        sphere_mesh,
        box_material: spawned.material("box").unwrap_or_default(),
        bouncy_material: spawned.material("bouncy_rubber").unwrap_or_default(),
        ice_material: spawned.material("ice").unwrap_or_default(),
        heavy_material: spawned.material("heavy_metal").unwrap_or_default(),
        ..Default::default()
    };
    resources.insert(physics_sandbox);

    // === Scene lights & enemies ===
    resources.insert(SceneLights(spawned.lights));
    resources.insert(Enemies(spawned.enemies));

    // === Upload textures, create GPU buffers, and cache material bind groups ===
    let texture_views = {
        let mut texture_views = HashMap::new();
        let mut renderer = resources.expect_mut::<Renderer>();
        for (id, texture) in asset_manager.textures.get_all() {
            let (_, view) = renderer.upload_texture(texture);
            texture_views.insert(*id, view);
        }
        for (_, mesh) in asset_manager.meshes.get_all_mut() {
            mesh.create_buffers(&renderer.device);
        }
        for (id, material) in asset_manager.materials.get_all() {
            let handle = Handle::<Material>::new(*id);
            let albedo = material.albedo_map.and_then(|h| texture_views.get(&h.id));
            let normal = material.normal_map.and_then(|h| texture_views.get(&h.id));
            let emissive = material.emissive_map.and_then(|h| texture_views.get(&h.id));
            renderer.get_or_create_material_bind_group(handle, material, albedo, normal, emissive);
        }

        let ibl = crate::renderer::ibl::generate_procedural(&renderer.device, &renderer.queue);
        renderer.set_ibl(ibl);

        texture_views
    };
    resources.insert(TextureViews(texture_views));

    // === Audio ===
    let audio_system = match crate::audio::AudioSystem::new() {
        Ok(audio) => Some(audio),
        Err(e) => {
            log::warn!("Audio init failed: {}", e);
            None
        }
    };
    resources.insert(audio_system);

    resources.insert(asset_manager);
    resources.insert(physics_world);
}

fn solid_material(name: &str, color: [f32; 3], roughness: f32, metallic: f32) -> Material {
    Material {
        name: name.to_string(),
        albedo_factor: [color[0], color[1], color[2], 1.0],
        metallic,
        roughness,
        emissive_factor: [0.0; 3],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    }
}
