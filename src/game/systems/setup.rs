use crate::asset::{AssetManager, Handle, Material, Mesh, ProceduralGenerator};
use crate::core::{EngineWorld, Name, Resources, Schedule, Stage, Transform};
use crate::game::{EnemySpawner, Player, Weapon, WeaponModel};
use crate::physics::{PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Camera, Light, RenderMesh, Renderer};
use glam::{Quat, Vec3};
use std::collections::HashMap;

use super::{
    Enemies, MenuState, MouseLocked, MuzzleFlashTimer, PlayerBody, SceneLights, TextureViews,
    WeaponFeedbackAssets,
};

pub fn register(schedule: &mut Schedule) {
    schedule.add_system(Stage::Startup, setup_scene);
}

fn setup_scene(world: &mut EngineWorld, resources: &Resources) {
    // === Camera & player tuning ===
    {
        let mut camera = resources.expect_mut::<Camera>();
        camera.position = Vec3::new(0.0, 2.0, 5.0);
    }
    {
        let mut player = resources.expect_mut::<Player>();
        player.camera_controller.move_speed = 8.0;
        player.camera_controller.mouse_sensitivity = 0.002;
        player.height = 1.6;
    }

    let mut asset_manager = resources
        .remove::<AssetManager>()
        .expect("AssetManager missing");
    let mut physics_world = resources
        .remove::<PhysicsWorld>()
        .expect("PhysicsWorld missing");

    let cube_mesh = asset_manager
        .meshes
        .insert(ProceduralGenerator::create_cube());
    let sphere_mesh = asset_manager
        .meshes
        .insert(ProceduralGenerator::create_sphere(16, 8));
    let cylinder_mesh = asset_manager
        .meshes
        .insert(ProceduralGenerator::create_cylinder(24));
    let rifle_mesh = asset_manager
        .meshes
        .insert(ProceduralGenerator::create_rifle());

    // === Player physics body ===
    let player_rb = {
        let camera = resources.expect::<Camera>();
        let collider = crate::physics::PhysicsShape::Capsule {
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

    // === Weapon setup ===
    let weapon_mesh_handle = rifle_mesh;
    let weapon_material_handle = asset_manager
        .materials
        .insert(Material::metal([0.18, 0.2, 0.22]));

    let mut weapon_model = WeaponModel::new(weapon_mesh_handle, weapon_material_handle);
    weapon_model.position = Vec3::new(0.36, -0.30, -0.58);
    weapon_model.scale = Vec3::splat(0.9);

    resources.insert(Weapon::new("Rifle", 25.0, 10.0, 30));
    resources.insert(weapon_model);

    // === Materials ===
    let enemy_material = asset_manager.materials.insert(Material {
        name: "Enemy".to_string(),
        albedo_factor: [0.9, 0.1, 0.1, 1.0],
        metallic: 0.2,
        roughness: 0.6,
        emissive_factor: [0.0; 3],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    let ground_material = asset_manager.materials.insert(Material {
        name: "Ground".to_string(),
        albedo_factor: [0.3, 0.5, 0.3, 1.0],
        metallic: 0.0,
        roughness: 0.9,
        emissive_factor: [0.0; 3],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    let wall_material = asset_manager.materials.insert(Material {
        name: "Wall".to_string(),
        albedo_factor: [0.6, 0.6, 0.7, 1.0],
        metallic: 0.1,
        roughness: 0.5,
        emissive_factor: [0.0; 3],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    let box_material = asset_manager.materials.insert(Material {
        name: "Box".to_string(),
        albedo_factor: [0.8, 0.6, 0.2, 1.0],
        metallic: 0.0,
        roughness: 0.7,
        emissive_factor: [0.0; 3],
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    });
    let light_material =
        asset_manager
            .materials
            .insert(solid_material("Light Box", [0.95, 0.85, 0.25], 0.65, 0.0));
    let heavy_material = asset_manager.materials.insert(solid_material(
        "Heavy Metal",
        [0.18, 0.22, 0.28],
        0.25,
        0.8,
    ));
    let bouncy_material = asset_manager.materials.insert(solid_material(
        "Bouncy Rubber",
        [0.25, 0.95, 0.35],
        0.45,
        0.0,
    ));
    let ice_material =
        asset_manager
            .materials
            .insert(solid_material("Ice", [0.45, 0.85, 1.0], 0.12, 0.0));
    let ramp_material =
        asset_manager
            .materials
            .insert(solid_material("Ramp", [0.55, 0.52, 0.46], 0.8, 0.0));
    let target_material = asset_manager.materials.insert(solid_material(
        "Knockdown Target",
        [0.9, 0.18, 0.16],
        0.55,
        0.0,
    ));
    let bullet_hole_material = asset_manager.materials.insert(solid_material(
        "Bullet Hole",
        [0.015, 0.012, 0.01],
        0.95,
        0.0,
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

    let mut physics_sandbox = crate::physics::sandbox::PhysicsSandbox {
        cube_mesh,
        sphere_mesh,
        box_material,
        ..Default::default()
    };
    physics_sandbox.bouncy_material = bouncy_material;
    physics_sandbox.ice_material = ice_material;
    physics_sandbox.heavy_material = heavy_material;

    spawn_physics_playground(
        world,
        &mut physics_world,
        &mut physics_sandbox,
        PlaygroundAssets {
            cube_mesh,
            sphere_mesh,
            cylinder_mesh,
            light_material,
            heavy_material,
            bouncy_material,
            ice_material,
            ramp_material,
            target_material,
        },
    );
    resources.insert(physics_sandbox);
    resources.insert(WeaponFeedbackAssets {
        bullet_hole_mesh: cube_mesh,
        bullet_hole_material,
        impact_mesh: sphere_mesh,
        impact_material: muzzle_flash_material,
        muzzle_flash_mesh: sphere_mesh,
        muzzle_flash_material,
    });

    // === Ground ===
    let ground_entity = world.spawn();
    world.add_component(
        ground_entity,
        Transform::new(
            Vec3::new(0.0, -0.5, 0.0),
            Quat::IDENTITY,
            Vec3::new(100.0, 1.0, 100.0),
        ),
    );
    world.add_component(
        ground_entity,
        RenderMesh {
            mesh: cube_mesh,
            material: ground_material,
        },
    );
    let ground_collider = crate::physics::PhysicsShape::Cuboid {
        half_extents: Vec3::new(50.0, 0.5, 50.0),
    };
    let (ground_rb, ground_col) = physics_world.add_static_body(
        Vec3::new(0.0, -0.5, 0.0),
        ground_collider.to_rapier_collider(),
    );
    world.add_component(
        ground_entity,
        crate::physics::PhysicsBody::new(ground_rb, ground_col, true),
    );
    physics_world.register_entity(ground_col, ground_entity);

    // === Walls ===
    let wall_positions = [
        (Vec3::new(10.0, 2.0, 0.0), Vec3::new(0.5, 4.0, 20.0)),
        (Vec3::new(-10.0, 2.0, 0.0), Vec3::new(0.5, 4.0, 20.0)),
        (Vec3::new(0.0, 2.0, 10.0), Vec3::new(20.0, 4.0, 0.5)),
        (Vec3::new(0.0, 2.0, -10.0), Vec3::new(20.0, 4.0, 0.5)),
    ];
    for (pos, scale) in wall_positions.iter() {
        let wall = world.spawn();
        world.add_component(wall, Transform::new(*pos, Quat::IDENTITY, *scale));
        world.add_component(
            wall,
            RenderMesh {
                mesh: cube_mesh,
                material: wall_material,
            },
        );
        let wall_collider = crate::physics::PhysicsShape::Cuboid {
            half_extents: *scale * 0.5,
        };
        let (wall_rb, wall_col) =
            physics_world.add_static_body(*pos, wall_collider.to_rapier_collider());
        world.add_component(
            wall,
            crate::physics::PhysicsBody::new(wall_rb, wall_col, true),
        );
        physics_world.register_entity(wall_col, wall);
    }

    // === Boxes ===
    let box_positions = [
        Vec3::new(3.0, 0.5, 3.0),
        Vec3::new(-3.0, 0.5, -3.0),
        Vec3::new(3.0, 0.5, -3.0),
        Vec3::new(-3.0, 0.5, 3.0),
        Vec3::new(0.0, 0.5, -5.0),
    ];
    for pos in box_positions.iter() {
        let box_entity = world.spawn();
        world.add_component(box_entity, Transform::new(*pos, Quat::IDENTITY, Vec3::ONE));
        world.add_component(
            box_entity,
            RenderMesh {
                mesh: cube_mesh,
                material: box_material,
            },
        );
        let box_collider = crate::physics::PhysicsShape::Cuboid {
            half_extents: Vec3::splat(0.5),
        };
        let (box_rb, box_col) =
            physics_world.add_dynamic_body(*pos, box_collider.to_rapier_collider(), 1.0);
        world.add_component(
            box_entity,
            crate::physics::PhysicsBody::new(box_rb, box_col, false),
        );
        physics_world.register_entity(box_col, box_entity);
    }

    // === Lights ===
    let scene_lights = vec![
        Light::directional(Vec3::new(1.0, -1.0, 0.5), [1.0, 0.95, 0.9], 2.0),
        Light::point(Vec3::new(5.0, 3.0, -3.0), [1.0, 0.6, 0.3], 30.0, 12.0),
        Light::point(Vec3::new(-5.0, 3.0, -3.0), [0.3, 0.6, 1.0], 30.0, 12.0),
        Light::point(Vec3::new(0.0, 4.0, 5.0), [0.3, 1.0, 0.5], 25.0, 10.0),
    ];

    // === Enemies ===
    let mut enemy_spawner = EnemySpawner::new();
    enemy_spawner.spawn_points = vec![
        Vec3::new(5.0, 1.0, 5.0),
        Vec3::new(-5.0, 1.0, -5.0),
        Vec3::new(5.0, 1.0, -5.0),
    ];
    enemy_spawner.waypoints = vec![
        Vec3::new(5.0, 1.0, 5.0),
        Vec3::new(5.0, 1.0, -5.0),
        Vec3::new(-5.0, 1.0, -5.0),
        Vec3::new(-5.0, 1.0, 5.0),
    ];
    enemy_spawner.enemy_mesh = Some(sphere_mesh);
    enemy_spawner.enemy_material = Some(enemy_material);
    let enemies = enemy_spawner.spawn_enemies(world, &mut physics_world);

    resources.insert(SceneLights(scene_lights));
    resources.insert(Enemies(enemies));

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
            let handle = crate::asset::Handle::<Material>::new(*id);
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

#[derive(Clone, Copy)]
struct PlaygroundAssets {
    cube_mesh: Handle<Mesh>,
    sphere_mesh: Handle<Mesh>,
    cylinder_mesh: Handle<Mesh>,
    light_material: Handle<Material>,
    heavy_material: Handle<Material>,
    bouncy_material: Handle<Material>,
    ice_material: Handle<Material>,
    ramp_material: Handle<Material>,
    target_material: Handle<Material>,
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

fn spawn_physics_playground(
    world: &mut EngineWorld,
    physics_world: &mut PhysicsWorld,
    sandbox: &mut crate::physics::sandbox::PhysicsSandbox,
    assets: PlaygroundAssets,
) {
    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Slope Ramp",
        assets.cube_mesh,
        assets.ramp_material,
        Transform::new(
            Vec3::new(-6.2, 0.55, -2.8),
            Quat::from_rotation_x(-0.38),
            Vec3::new(7.0, 0.9, 3.2),
        ),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(3.5, 0.45, 1.6),
        },
        0.0,
        PhysicsMaterial::DEFAULT,
        Vec3::ZERO,
    );

    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Ice Lane",
        assets.cube_mesh,
        assets.ice_material,
        Transform::new(
            Vec3::new(0.8, 0.04, -6.4),
            Quat::IDENTITY,
            Vec3::new(8.5, 0.12, 2.0),
        ),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(4.25, 0.06, 1.0),
        },
        0.0,
        PhysicsMaterial::ICE,
        Vec3::ZERO,
    );

    for i in 0..4 {
        spawn_playground_prop(
            world,
            physics_world,
            sandbox,
            "Bouncy Ball",
            assets.sphere_mesh,
            assets.bouncy_material,
            Transform::new(
                Vec3::new(-8.2 + i as f32 * 0.75, 3.0 + i as f32 * 0.18, -3.3),
                Quat::IDENTITY,
                Vec3::splat(0.7),
            ),
            PhysicsShape::Sphere { radius: 0.35 },
            0.45,
            PhysicsMaterial::RUBBER,
            Vec3::ZERO,
        );
    }

    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Light Box",
        assets.cube_mesh,
        assets.light_material,
        Transform::new(Vec3::new(-2.5, 0.55, 1.8), Quat::IDENTITY, Vec3::splat(0.8)),
        PhysicsShape::Cuboid {
            half_extents: Vec3::splat(0.4),
        },
        0.2,
        PhysicsMaterial::DEFAULT,
        Vec3::ZERO,
    );
    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Heavy Box",
        assets.cube_mesh,
        assets.heavy_material,
        Transform::new(Vec3::new(-1.2, 0.55, 1.8), Quat::IDENTITY, Vec3::splat(0.8)),
        PhysicsShape::Cuboid {
            half_extents: Vec3::splat(0.4),
        },
        8.0,
        PhysicsMaterial::METAL,
        Vec3::ZERO,
    );

    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Ice Puck",
        assets.cylinder_mesh,
        assets.ice_material,
        Transform::new(
            Vec3::new(-3.4, 0.25, -6.4),
            Quat::IDENTITY,
            Vec3::new(0.8, 0.25, 0.8),
        ),
        PhysicsShape::Cylinder {
            radius: 0.4,
            half_height: 0.125,
        },
        0.8,
        PhysicsMaterial::ICE,
        Vec3::new(6.5, 0.0, 0.0),
    );
    spawn_playground_prop(
        world,
        physics_world,
        sandbox,
        "Rough Puck",
        assets.cylinder_mesh,
        assets.ramp_material,
        Transform::new(
            Vec3::new(-3.4, 0.18, -4.2),
            Quat::IDENTITY,
            Vec3::new(0.8, 0.25, 0.8),
        ),
        PhysicsShape::Cylinder {
            radius: 0.4,
            half_height: 0.125,
        },
        0.8,
        PhysicsMaterial::DEFAULT,
        Vec3::new(6.5, 0.0, 0.0),
    );

    for i in 0..8 {
        spawn_playground_prop(
            world,
            physics_world,
            sandbox,
            "Knockdown Target",
            assets.cube_mesh,
            assets.target_material,
            Transform::new(
                Vec3::new(5.0, 0.85, -7.4 + i as f32 * 0.32),
                Quat::IDENTITY,
                Vec3::new(0.16, 1.6, 0.28),
            ),
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(0.08, 0.8, 0.14),
            },
            0.35,
            PhysicsMaterial {
                friction: 0.6,
                restitution: 0.1,
            },
            Vec3::ZERO,
        );
    }
}

fn spawn_playground_prop(
    world: &mut EngineWorld,
    physics_world: &mut PhysicsWorld,
    sandbox: &mut crate::physics::sandbox::PhysicsSandbox,
    name: &'static str,
    mesh: Handle<Mesh>,
    material: Handle<Material>,
    transform: Transform,
    shape: PhysicsShape,
    mass: f32,
    physics_material: PhysicsMaterial,
    initial_impulse: Vec3,
) -> hecs::Entity {
    let entity = sandbox.spawn_prop_with_physics_material(
        world,
        physics_world,
        mesh,
        material,
        transform,
        shape,
        mass,
        physics_material,
    );
    world.add_component(entity, Name(name));
    if initial_impulse.length_squared() > 0.0 {
        if let Some(body) = world.get_component::<crate::physics::PhysicsBody>(entity) {
            physics_world.apply_impulse(body.rigid_body_handle, initial_impulse);
        }
    }
    entity
}
