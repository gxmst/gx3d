use crate::asset::{AssetManager, Handle, Material, Mesh, ProceduralGenerator};
use crate::core::{EngineWorld, Resources, Schedule, Stage};
use crate::game::scene::{spawn_scene, Scene};
use crate::game::{Player, Weapon, WeaponModel};
use crate::physics::{PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Camera, Renderer};
use glam::Vec3;
use std::collections::HashMap;

use super::{
    EnemyFlashMaterial, HitMarkerTimer, MenuState, MouseLocked, MuzzleFlashTimer,
    PhysicsDebugState, PlayerBody, SceneLights, WeaponFeedbackAssets,
};

/// The default scene, compiled into the binary as a fallback so the engine can
/// always start even if the on-disk scene file is missing or malformed.
const EMBEDDED_SCENE: &str = include_str!("../../../assets/scenes/dust2.json");

/// Path (relative to the working directory) of the scene loaded at startup.
/// Editing this file lets you change the level without recompiling.
pub const DEFAULT_SCENE_PATH: &str = "assets/scenes/dust2.json";

/// Scene file requested on the command line (parsed and validated in `main`).
/// `None` selects [`DEFAULT_SCENE_PATH`].
pub struct RequestedScenePath(pub Option<std::path::PathBuf>);

pub fn register(schedule: &mut Schedule) {
    schedule.add_system(Stage::Startup, setup_scene);
}

/// Load the startup scene: prefer the on-disk file (so edits take effect
/// without recompiling), and fall back to the embedded copy if it is missing
/// or fails to parse. Either way the engine starts with a valid scene.
fn load_scene(resources: &Resources) -> Scene {
    let scene_path = resources
        .get::<RequestedScenePath>()
        .and_then(|requested| requested.0.clone())
        .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_SCENE_PATH));
    match std::fs::read_to_string(&scene_path) {
        Ok(text) => match Scene::from_json(&text) {
            Ok(scene) => {
                log::info!("Loaded scene from {}", scene_path.display());
                return scene;
            }
            Err(e) => {
                log::error!(
                    "Failed to parse {}: {e}. Falling back to embedded scene.",
                    scene_path.display()
                );
                show_toast(resources, format!("场景解析失败，已回退内置场景：{e}"));
            }
        },
        Err(e) => {
            log::info!(
                "No scene file at {} ({e}). Using embedded scene.",
                scene_path.display()
            );
            show_toast(
                resources,
                format!("找不到场景文件 {}，已使用内置场景", scene_path.display()),
            );
        }
    }
    // The embedded scene is authored alongside the code and is expected to
    // always parse; if it does not, that is a build-time bug worth surfacing.
    Scene::from_json(EMBEDDED_SCENE).expect("embedded scene must parse")
}

fn setup_scene(world: &mut EngineWorld, resources: &Resources) {
    let scene = load_scene(resources);
    if !resources.contains::<super::Toast>() {
        resources.insert(super::Toast::default());
    }
    resources.insert(super::PlayerHealth::default());

    let mut asset_manager = resources.expect_mut::<AssetManager>();
    let mut physics_world = resources.expect_mut::<PhysicsWorld>();

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
        player.set_spawn_position(Vec3::from_array(spawned.player.position));
    }

    // === Player physics body (engine plumbing, not scene content) ===
    let player_rb = {
        let camera = resources.expect::<Camera>();
        let player_height = spawned.player.height.max(0.8);
        let radius = (player_height * 0.1875).clamp(0.28, 0.36);
        let collider = PhysicsShape::Capsule {
            radius,
            half_height: (player_height * 0.5 - radius).max(0.1),
        };
        let (rb, _col) = physics_world.add_kinematic_body(
            camera.position,
            collider.to_rapier_collider_with_material(PhysicsMaterial {
                friction: 0.0,
                restitution: 0.0,
            }),
        );
        if let Some(rb_ref) = physics_world.rigid_body_set.get_mut(rb) {
            rb_ref.lock_rotations(true, true);
            rb_ref.enable_ccd(true);
            rb_ref.set_soft_ccd_prediction(0.2);
        }
        rb
    };
    resources.insert(PlayerBody(player_rb));
    resources.insert(MouseLocked(true));
    {
        // Restore persisted user settings. The menu state is rebuilt per scene
        // load, so the saved values are the single source of truth.
        let config = resources.expect::<crate::core::UserConfig>();
        let mut menu = MenuState::default();
        menu.0.selected_resolution = config.resolution_index;
        menu.0.god_mode_enabled = config.god_mode_enabled;
        menu.0.sensitivity_index = config.sensitivity_index;
        menu.0.weather = config.weather;
        if let Some(library) = resources.get::<crate::game::scene::SceneLibrary>() {
            menu.0.scene_names = library.0.iter().map(|entry| entry.name.clone()).collect();
            if let Some(current) = resources.get::<crate::core::CurrentScenePath>() {
                menu.0.current_scene = library.index_of(&current.0);
            }
        }
        resources.insert(crate::game::systems::ViewModeState {
            god_mode_enabled: config.god_mode_enabled,
            ..Default::default()
        });
        let mut player = resources.expect_mut::<Player>();
        player.camera_controller.mouse_sensitivity *= config.sensitivity_multiplier();
        resources.insert(menu);
    }
    resources.insert(MuzzleFlashTimer(0.0));
    resources.insert(HitMarkerTimer::default());
    resources.insert(PhysicsDebugState::default());
    resources.insert(crate::game::InteractionFocus::default());

    // === Weapon (engine plumbing) ===
    // The rifle mesh comes from the scene; its material is engine-defined.
    let rifle_mesh = mesh_or_fallback(
        &mut asset_manager,
        spawned.mesh("rifle"),
        "rifle",
        ProceduralGenerator::create_rifle,
    );
    // One distinct first-person mesh per catalog slot (pistol / SMG / rifle /
    // marksman / shotgun), index-aligned with WEAPON_CATALOG.
    let weapon_meshes = crate::game::systems::WeaponMeshes(vec![
        asset_manager
            .meshes
            .insert(ProceduralGenerator::create_pistol()),
        asset_manager
            .meshes
            .insert(ProceduralGenerator::create_smg()),
        rifle_mesh,
        asset_manager
            .meshes
            .insert(ProceduralGenerator::create_marksman_rifle()),
        asset_manager
            .meshes
            .insert(ProceduralGenerator::create_shotgun()),
    ]);
    resources.insert(weapon_meshes);
    let weapon_material = asset_manager
        .materials
        .insert(Material::metal([0.18, 0.2, 0.22]));
    let mut weapon_model = WeaponModel::new(rifle_mesh, weapon_material);
    weapon_model.position = Vec3::new(0.36, -0.30, -0.58);
    weapon_model.scale = Vec3::splat(0.9);
    resources.insert(Weapon::from_spec(crate::game::weapon::SANDBOX_WEAPON_INDEX));
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
        water: false,
    });
    let cube_mesh = mesh_or_fallback(
        &mut asset_manager,
        spawned.mesh("cube"),
        "cube",
        ProceduralGenerator::create_cube,
    );
    let sphere_mesh =
        mesh_or_fallback(&mut asset_manager, spawned.mesh("sphere"), "sphere", || {
            ProceduralGenerator::create_sphere(32, 16)
        });
    let cylinder_mesh = mesh_or_fallback(
        &mut asset_manager,
        spawned.mesh("cylinder"),
        "cylinder",
        || ProceduralGenerator::create_cylinder(32),
    );
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
        water: false,
    });
    resources.insert(EnemyFlashMaterial(enemy_flash_material));

    // === Physics sandbox shared handles (for runtime G/B/H spawning) ===
    let box_material =
        material_or_fallback(&mut asset_manager, spawned.material("box"), "box", || {
            solid_material("Fallback Box", [0.58, 0.36, 0.18], 0.78, 0.0)
        });
    let bouncy_material = material_or_fallback(
        &mut asset_manager,
        spawned.material("bouncy_rubber"),
        "bouncy_rubber",
        || solid_material("Fallback Bouncy Rubber", [0.82, 0.12, 0.16], 0.88, 0.0),
    );
    let ice_material =
        material_or_fallback(&mut asset_manager, spawned.material("ice"), "ice", || {
            solid_material("Fallback Ice", [0.36, 0.72, 0.96], 0.16, 0.0)
        });
    let heavy_material = material_or_fallback(
        &mut asset_manager,
        spawned.material("heavy_metal"),
        "heavy_metal",
        || solid_material("Fallback Heavy Metal", [0.22, 0.25, 0.28], 0.32, 0.92),
    );
    let physics_sandbox = crate::physics::sandbox::PhysicsSandbox {
        cube_mesh,
        sphere_mesh,
        box_material,
        bouncy_material,
        ice_material,
        heavy_material,
        ..Default::default()
    };
    resources.insert(physics_sandbox);
    resources.insert(crate::game::systems::ragdoll::RagdollState::default());
    resources.insert(crate::game::systems::bot_weapon::BotShotFlashes::default());
    if !resources.contains::<crate::game::systems::debug_panel::DebugPanelState>() {
        resources.insert(crate::game::systems::debug_panel::DebugPanelState::default());
    }
    if !resources.contains::<crate::game::systems::render::ExposureSetting>() {
        resources.insert(crate::game::systems::render::ExposureSetting::default());
    }

    // === Weather (engine plumbing; selected kind persists in config) ===
    {
        let rain_material = asset_manager.materials.insert(Material {
            name: "Rain Streak".to_string(),
            albedo_factor: [0.62, 0.72, 0.86, 0.34],
            metallic: 0.0,
            roughness: 0.2,
            emissive_factor: [0.12, 0.16, 0.22],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        });
        let snow_material = asset_manager.materials.insert(Material {
            name: "Snow Flake".to_string(),
            albedo_factor: [0.95, 0.96, 0.99, 0.9],
            metallic: 0.0,
            roughness: 0.85,
            emissive_factor: [0.28, 0.29, 0.32],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        });
        let selected = resources.expect::<crate::core::UserConfig>().weather;
        resources.insert(crate::game::systems::weather::WeatherState::new(
            rain_material,
            snow_material,
            cube_mesh,
            selected,
        ));
    }

    // === Match mode (optional) ===
    // The player takes the first team-A spawn in match scenes.
    if let Some(match_state) = spawned.match_state {
        {
            let mut camera = resources.expect_mut::<Camera>();
            camera.position = match_state.player_spawn;
        }
        {
            let mut player = resources.expect_mut::<Player>();
            player.set_spawn_position(match_state.player_spawn);
        }
        let player_body = resources.expect::<PlayerBody>().0;
        physics_world.teleport_body(player_body, match_state.player_spawn);
        resources.insert(match_state);
        resources.insert(super::buy_menu::BuyState::new(true));
        // Match rounds start on the default pistol (with its own mesh).
        super::buy_menu::equip_weapon(resources, crate::game::weapon::DEFAULT_WEAPON_INDEX);
    } else {
        resources.remove::<crate::game::systems::match_mode::MatchState>();
        resources.insert(super::buy_menu::BuyState::new(false));
    }

    // === Scene lights ===
    resources.insert(SceneLights(spawned.lights));

    // === Water surface (physics showcase; None when the scene has none) ===
    resources.insert::<Option<crate::game::WaterSurface>>(spawned.water);

    // === Upload textures, create GPU buffers, and cache material bind groups ===
    {
        // Normal and metallic-roughness maps hold linear data; only textures
        // used as albedo/emissive may be uploaded as sRGB. Classify by how
        // materials reference each texture (data usage wins on conflict).
        let mut data_texture_ids = std::collections::HashSet::new();
        for (_, material) in asset_manager.materials.get_all() {
            if let Some(handle) = material.normal_map {
                data_texture_ids.insert(handle.id);
            }
            if let Some(handle) = material.metallic_roughness_map {
                data_texture_ids.insert(handle.id);
            }
        }

        let mut texture_views = HashMap::new();
        let mut renderer = resources.expect_mut::<Renderer>();
        for (id, texture) in asset_manager.textures.get_all() {
            let srgb = !data_texture_ids.contains(id);
            let (_, view) = renderer.upload_texture(texture, srgb);
            texture_views.insert(*id, view);
        }
        for (_, mesh) in asset_manager.meshes.get_all_mut() {
            mesh.create_buffers(&renderer.device);
        }
        for (id, material) in asset_manager.materials.get_all() {
            let handle = Handle::<Material>::new(*id);
            let views = crate::renderer::MaterialTextureViews {
                albedo: material.albedo_map.and_then(|h| texture_views.get(&h.id)),
                normal: material.normal_map.and_then(|h| texture_views.get(&h.id)),
                emissive: material.emissive_map.and_then(|h| texture_views.get(&h.id)),
                metallic_roughness: material
                    .metallic_roughness_map
                    .and_then(|h| texture_views.get(&h.id)),
            };
            renderer.get_or_create_material_bind_group(handle, material, views);
        }

        let ibl = crate::renderer::ibl::generate_procedural(&renderer.device, &renderer.queue);
        renderer.set_ibl(ibl);
    }

    // === Audio ===
    let audio_system = match crate::audio::AudioSystem::new() {
        Ok(audio) => Some(audio),
        Err(e) => {
            log::warn!("Audio init failed: {}", e);
            None
        }
    };
    resources.insert(audio_system);

    // Build Rapier's broad phase before the first character-controller tick,
    // and let initially overlapping dynamic props settle by one fixed step.
    physics_world.step();
}

/// Queue a HUD toast, inserting the resource if setup runs before it exists.
fn show_toast(resources: &Resources, message: String) {
    if let Some(mut toast) = resources.get_mut::<super::Toast>() {
        toast.show(message, 5.0);
    } else {
        let mut toast = super::Toast::default();
        toast.show(message, 5.0);
        resources.insert(toast);
    }
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
        water: false,
    }
}

fn mesh_or_fallback(
    assets: &mut AssetManager,
    scene_handle: Option<Handle<Mesh>>,
    name: &str,
    create_fallback: impl FnOnce() -> Mesh,
) -> Handle<Mesh> {
    match scene_handle {
        Some(handle) if assets.meshes.get(handle).is_some() => handle,
        Some(handle) => {
            log::warn!(
                "Scene mesh `{name}` points to missing asset handle {}; generating fallback",
                handle.id
            );
            assets.meshes.insert(create_fallback())
        }
        None => {
            log::warn!("Scene mesh `{name}` is unavailable; generating fallback");
            assets.meshes.insert(create_fallback())
        }
    }
}

fn material_or_fallback(
    assets: &mut AssetManager,
    scene_handle: Option<Handle<Material>>,
    name: &str,
    create_fallback: impl FnOnce() -> Material,
) -> Handle<Material> {
    match scene_handle {
        Some(handle) if assets.materials.get(handle).is_some() => handle,
        Some(handle) => {
            log::warn!(
                "Scene material `{name}` points to missing asset handle {}; generating fallback",
                handle.id
            );
            assets.materials.insert(create_fallback())
        }
        None => {
            log::warn!("Scene material `{name}` is unavailable; generating fallback");
            assets.materials.insert(create_fallback())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_mesh_gets_a_real_fallback_handle() {
        let mut assets = AssetManager::new();

        let handle = mesh_or_fallback(&mut assets, None, "cube", ProceduralGenerator::create_cube);

        let mesh = assets.meshes.get(handle).expect("fallback mesh must exist");
        assert_eq!(mesh.name, "Cube");
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn dangling_mesh_handle_is_replaced() {
        let mut assets = AssetManager::new();

        let handle = mesh_or_fallback(&mut assets, Some(Handle::new(99)), "sphere", || {
            ProceduralGenerator::create_sphere(8, 4)
        });

        assert_ne!(handle.id, 99);
        assert!(assets.meshes.get(handle).is_some());
    }

    #[test]
    fn missing_material_gets_a_real_fallback_handle() {
        let mut assets = AssetManager::new();

        let handle = material_or_fallback(&mut assets, None, "box", || {
            solid_material("Fallback Box", [0.5, 0.3, 0.1], 0.8, 0.0)
        });

        let material = assets
            .materials
            .get(handle)
            .expect("fallback material must exist");
        assert_eq!(material.name, "Fallback Box");
        assert!(material.albedo_factor.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn existing_asset_handles_are_preserved() {
        let mut assets = AssetManager::new();
        let existing_mesh = assets.meshes.insert(ProceduralGenerator::create_cube());
        let existing_material = assets.materials.insert(Material::gray());

        let mesh = mesh_or_fallback(&mut assets, Some(existing_mesh), "cube", || {
            panic!("mesh fallback must not run")
        });
        let material = material_or_fallback(&mut assets, Some(existing_material), "box", || {
            panic!("material fallback must not run")
        });

        assert_eq!(mesh.id, existing_mesh.id);
        assert_eq!(material.id, existing_material.id);
    }
}
