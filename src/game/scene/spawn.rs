//! Instantiation of a [`Scene`] description into the live ECS world.
//!
//! This turns plain data (meshes, materials, entities, lights, enemies) into
//! actual assets, rigid bodies, and ECS entities. Engine plumbing that scenes
//! only *reference* by name (procedural meshes, GPU upload, weapons) is wired
//! up by the caller in `setup.rs` using the name → handle maps returned here.

use std::collections::{HashMap, HashSet};

use glam::{EulerRot, Quat, Vec3};
use hecs::Entity;

use crate::asset::{AssetManager, GltfLoader, Handle, Material, Mesh, ProceduralGenerator};
use crate::core::{EngineWorld, Name, Transform};
use crate::game::{EnemySpawner, Explosive, ToggleDoor};
use crate::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Light, RenderMesh};

use super::{
    EnemyDesc, EntityDesc, LightDesc, MatchDesc, MaterialDesc, MeshSource, PhysicsDesc, PlayerDesc,
    Scene, ShapeDesc, StructureDesc, SurfaceDesc, SurfacePreset, TornadoDesc, TransformDesc,
    WaterDesc,
};

/// The result of instantiating a [`Scene`]: name → handle maps so the caller
/// can resolve engine-side assets (weapon mesh, hit-feedback materials, the
/// sandbox's shared handles) that scenes reference by name, plus the lights,
/// enemies, and player config the scene declared.
pub struct SpawnedScene {
    /// Mesh handles keyed by the name declared in the scene.
    pub meshes: HashMap<String, Handle<Mesh>>,
    /// Material handles keyed by the name declared in the scene.
    pub materials: HashMap<String, Handle<Material>>,
    /// Lights, ready to upload to the GPU each frame.
    pub lights: Vec<Light>,
    /// Spawned enemy entities.
    pub enemies: Vec<Entity>,
    /// Player spawn / tuning configuration from the scene.
    pub player: PlayerDesc,
    /// Water surface instantiated from the scene's `water` block, to be
    /// installed as a resource by the caller.
    pub water: Option<crate::game::WaterSurface>,
    /// Match state when the scene declares a `match_mode` block.
    pub match_state: Option<crate::game::systems::match_mode::MatchState>,
}

impl SpawnedScene {
    /// Look up a mesh handle by scene name, logging if it is missing.
    pub fn mesh(&self, name: &str) -> Option<Handle<Mesh>> {
        let handle = self.meshes.get(name).copied();
        if handle.is_none() {
            log::warn!("Scene references unknown mesh `{name}`");
        }
        handle
    }

    /// Look up a material handle by scene name, logging if it is missing.
    pub fn material(&self, name: &str) -> Option<Handle<Material>> {
        let handle = self.materials.get(name).copied();
        if handle.is_none() {
            log::warn!("Scene references unknown material `{name}`");
        }
        handle
    }
}

/// Instantiate a scene: register its meshes/materials into `assets`, spawn its
/// entities and enemies into `world`/`physics`, and return the lookup maps and
/// scene-level data the caller needs.
pub fn spawn_scene(
    scene: &Scene,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
    assets: &mut AssetManager,
) -> SpawnedScene {
    for warning in scene.validation_warnings() {
        log::warn!("Scene validation: {warning}");
    }
    // --- Meshes -----------------------------------------------------------
    let mut meshes: HashMap<String, Handle<Mesh>> = HashMap::new();
    let mut gltf_materials: HashMap<String, Handle<Material>> = HashMap::new();
    let mut gltf_cache = HashMap::new();
    let mut failed_gltf_paths = HashSet::new();
    for desc in &scene.meshes {
        let handle = match &desc.source {
            MeshSource::Gltf { path, primitive } => {
                if !gltf_cache.contains_key(path) && !failed_gltf_paths.contains(path) {
                    match GltfLoader::load(path, assets) {
                        Ok(imported) => {
                            gltf_cache.insert(path.clone(), imported);
                        }
                        Err(error) => {
                            log::error!("Failed to load glTF `{path}`: {error}");
                            failed_gltf_paths.insert(path.clone());
                        }
                    }
                }
                let Some(imported) = gltf_cache.get(path) else {
                    continue;
                };
                let Some(handle) = imported.meshes.get(*primitive).copied() else {
                    log::error!(
                        "glTF `{path}` has no primitive {primitive}; skipping mesh `{}`",
                        desc.name
                    );
                    continue;
                };
                if let Some(material) = imported.mesh_materials.get(*primitive).copied() {
                    gltf_materials.insert(desc.name.clone(), material);
                }
                handle
            }
            source => assets.meshes.insert(build_mesh(source)),
        };
        if meshes.insert(desc.name.clone(), handle).is_some() {
            log::warn!("Scene declares duplicate mesh name `{}`", desc.name);
        }
    }

    // --- Materials --------------------------------------------------------
    let mut materials: HashMap<String, Handle<Material>> = HashMap::new();
    for desc in &scene.materials {
        let handle = assets.materials.insert(build_material(desc));
        if materials.insert(desc.name.clone(), handle).is_some() {
            log::warn!("Scene declares duplicate material name `{}`", desc.name);
        }
    }

    // --- Entities ---------------------------------------------------------
    for desc in &scene.entities {
        spawn_entity(desc, world, physics, &meshes, &materials, &gltf_materials);
    }

    // --- Lights -----------------------------------------------------------
    let lights = scene.lights.iter().map(build_light).collect();

    // --- Enemies / match bots --------------------------------------------
    // A match block supersedes plain patrol enemies.
    let (enemies, match_state) = if let Some(desc) = &scene.match_mode {
        (
            Vec::new(),
            spawn_match(desc, world, physics, &meshes, &materials),
        )
    } else {
        (
            scene
                .enemies
                .as_ref()
                .map(|desc| spawn_enemies(desc, world, physics, &meshes, &materials))
                .unwrap_or_default(),
            None,
        )
    };

    // --- Physics showcase effects ----------------------------------------
    if let Some(desc) = &scene.tornado {
        spawn_tornado(desc, world, assets, &materials);
    }
    let water = scene
        .water
        .as_ref()
        .and_then(|desc| spawn_water(desc, world, assets, &materials));
    for desc in &scene.structures {
        spawn_structure(desc, world, physics, &meshes, &materials);
    }

    SpawnedScene {
        meshes,
        materials,
        lights,
        enemies,
        player: sanitize_player(&scene.player),
        water,
        match_state,
    }
}

/// Spawn both bot teams and build the match state. The first team-A spawn is
/// reserved for the player.
fn spawn_match(
    desc: &MatchDesc,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
    meshes: &HashMap<String, Handle<Mesh>>,
    materials: &HashMap<String, Handle<Material>>,
) -> Option<crate::game::systems::match_mode::MatchState> {
    use crate::game::systems::match_mode::StatEntry;
    use crate::game::{BotBrain, EnemyAI, Team};
    let Some(mesh) = meshes.get(&desc.mesh).copied() else {
        log::warn!("Match references unknown mesh `{}`", desc.mesh);
        return None;
    };
    let team_a_material = materials.get(&desc.team_a_material).copied();
    let team_b_material = materials.get(&desc.team_b_material).copied();
    let (Some(material_a), Some(material_b)) = (team_a_material, team_b_material) else {
        log::warn!("Match team materials missing; match disabled");
        return None;
    };
    if desc.team_a_spawns.is_empty() || desc.team_b_spawns.is_empty() {
        log::warn!("Match requires spawns for both teams; match disabled");
        return None;
    }

    let spawn_bot = |world: &mut EngineWorld,
                     physics: &mut PhysicsWorld,
                     team: Team,
                     position: Vec3,
                     material: Handle<Material>,
                     stat_index: usize| {
        let entity = world.spawn();
        let mut transform = Transform::from_position(position);
        transform.scale = Vec3::new(0.8, 0.8, 0.8);
        world.add_component(entity, transform);
        // Movement/health via EnemyAI (waypoints assigned dynamically by the
        // match AI); combat memory via BotBrain.
        world.add_component(entity, EnemyAI::new(Vec::new(), 2.6, 100.0));
        world.add_component(entity, BotBrain::new(team, position, stat_index));
        world.add_component(entity, RenderMesh { mesh, material });
        let collider = PhysicsShape::Capsule {
            radius: 0.4,
            half_height: 0.8,
        };
        let (rb, col) = physics.add_kinematic_body(position, collider.to_rapier_collider());
        world.add_component(entity, PhysicsBody::new(rb, col, false));
        physics.register_entity(col, entity);
    };

    // Scoreboard rows: player first, then teammates, then opponents.
    let mut stats = vec![StatEntry {
        name: "你".to_string(),
        team: Team::Alpha,
        kills: 0,
        deaths: 0,
    }];
    // Team A: skip the first spawn (player slot).
    for (i, point) in desc.team_a_spawns.iter().skip(1).enumerate() {
        let position = finite_vec3(*point, Vec3::ZERO, 100_000.0);
        let stat_index = stats.len();
        stats.push(StatEntry {
            name: format!("队友 {}", i + 1),
            team: Team::Alpha,
            kills: 0,
            deaths: 0,
        });
        spawn_bot(
            world,
            physics,
            Team::Alpha,
            position,
            material_a,
            stat_index,
        );
    }
    for (i, point) in desc.team_b_spawns.iter().enumerate() {
        let position = finite_vec3(*point, Vec3::ZERO, 100_000.0);
        let stat_index = stats.len();
        stats.push(StatEntry {
            name: format!("敌方 {}", i + 1),
            team: Team::Bravo,
            kills: 0,
            deaths: 0,
        });
        spawn_bot(
            world,
            physics,
            Team::Bravo,
            position,
            material_b,
            stat_index,
        );
    }

    let waypoints = desc
        .waypoints
        .iter()
        .copied()
        .map(Vec3::from_array)
        .filter(|p| p.is_finite())
        .collect();
    let player_spawn = finite_vec3(desc.team_a_spawns[0], Vec3::ZERO, 100_000.0);
    let bomb_site = desc.bomb_site.map(|center| {
        (
            finite_vec3(center, Vec3::ZERO, 100_000.0),
            desc.bomb_site_radius.clamp(1.5, 30.0),
        )
    });
    Some(crate::game::systems::match_mode::MatchState::new(
        waypoints,
        player_spawn,
        desc.rounds_to_win,
        bomb_site,
        stats,
    ))
}

fn spawn_tornado(
    desc: &TornadoDesc,
    world: &mut EngineWorld,
    assets: &mut AssetManager,
    materials: &HashMap<String, Handle<Material>>,
) {
    let tornado = world.spawn();
    world.add_component(
        tornado,
        crate::game::Tornado {
            center: finite_vec3(desc.center, Vec3::ZERO, 100_000.0),
            radius: finite_clamped(desc.radius, super::default_tornado_radius(), 2.0, 60.0),
            height: finite_clamped(desc.height, super::default_tornado_height(), 4.0, 120.0),
            strength: finite_clamped(
                desc.strength,
                super::default_tornado_strength(),
                0.0,
                3_000.0,
            ),
            wander: finite_clamped(desc.wander, super::default_tornado_wander(), 0.0, 30.0),
            time: 0.0,
        },
    );

    // Visual funnel: a few dozen swirling dust motes. They reuse the scene's
    // `dust` material if declared, else a neutral gray.
    let dust_material = materials.get("dust").copied().unwrap_or_else(|| {
        assets.materials.insert(Material {
            name: "Tornado Dust".to_string(),
            albedo_factor: [0.45, 0.42, 0.38, 1.0],
            metallic: 0.0,
            roughness: 0.95,
            emissive_factor: [0.0; 3],
            albedo_map: None,
            normal_map: None,
            metallic_roughness_map: None,
            emissive_map: None,
            water: false,
        })
    });
    let dust_mesh = assets
        .meshes
        .insert(ProceduralGenerator::create_sphere(8, 5));
    for i in 0..48 {
        let mote = world.spawn();
        world.add_component(
            mote,
            crate::game::TornadoDust {
                seed: (i as f32) / 48.0,
            },
        );
        world.add_component(
            mote,
            Transform::new(
                finite_vec3(desc.center, Vec3::ZERO, 100_000.0),
                Quat::IDENTITY,
                Vec3::splat(0.2),
            ),
        );
        world.add_component(
            mote,
            RenderMesh {
                mesh: dust_mesh,
                material: dust_material,
            },
        );
    }
}

fn spawn_water(
    desc: &WaterDesc,
    world: &mut EngineWorld,
    assets: &mut AssetManager,
    materials: &HashMap<String, Handle<Material>>,
) -> Option<crate::game::WaterSurface> {
    let Some(material) = materials.get(&desc.material).copied() else {
        log::warn!("Water references unknown material `{}`", desc.material);
        return None;
    };
    let size = finite_clamped(desc.size, super::default_water_size(), 4.0, 2_000.0);
    let subdivisions = desc.subdivisions.clamp(8, 256);
    let center = finite_vec3(desc.center, Vec3::ZERO, 100_000.0);
    let amplitude = finite_clamped(desc.amplitude, super::default_water_amplitude(), 0.0, 4.0);

    // The plane is generated around the origin; bake the world offset into
    // the vertices so the wave functions can use world x/z directly.
    let mut mesh = ProceduralGenerator::create_plane(size, subdivisions);
    for vertex in &mut mesh.vertices {
        vertex.position[0] += center.x;
        vertex.position[1] += center.y;
        vertex.position[2] += center.z;
    }
    let base_vertices = mesh.vertices.clone();
    let mesh_handle = assets.meshes.insert(mesh);

    let entity = world.spawn();
    // Identity transform: vertices are already in world space.
    world.add_component(entity, Transform::from_position(Vec3::ZERO));
    world.add_component(
        entity,
        RenderMesh {
            mesh: mesh_handle,
            material,
        },
    );

    Some(crate::game::WaterSurface {
        mesh: mesh_handle,
        center,
        size,
        amplitude,
        time: 0.0,
        base_vertices,
    })
}

fn spawn_structure(
    desc: &StructureDesc,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
    meshes: &HashMap<String, Handle<Mesh>>,
    materials: &HashMap<String, Handle<Material>>,
) {
    let Some(mesh) = meshes.get(&desc.mesh).copied() else {
        log::warn!("Structure references unknown mesh `{}`", desc.mesh);
        return;
    };
    let Some(material) = materials.get(&desc.material).copied() else {
        log::warn!("Structure references unknown material `{}`", desc.material);
        return;
    };
    let half = Vec3::from_array(desc.block_half_extents).clamp(Vec3::splat(0.05), Vec3::splat(5.0));
    let base = finite_vec3(desc.position, Vec3::ZERO, 100_000.0);
    let [nx, ny, nz] = desc.blocks.map(|n| n.clamp(1, 24));
    let mass = finite_clamped(
        desc.block_mass,
        super::default_structure_block_mass(),
        1.0,
        500.0,
    );

    let mut spawned = 0usize;
    for iy in 0..ny {
        for ix in 0..nx {
            for iz in 0..nz {
                // Hollow structures only keep perimeter columns (walls).
                if desc.hollow && ix != 0 && ix != nx - 1 && iz != 0 && iz != nz - 1 {
                    continue;
                }
                let position = base
                    + Vec3::new(
                        (ix as f32 - (nx as f32 - 1.0) * 0.5) * half.x * 2.0,
                        half.y + iy as f32 * half.y * 2.0,
                        (iz as f32 - (nz as f32 - 1.0) * 0.5) * half.z * 2.0,
                    );
                let collider = PhysicsShape::Cuboid { half_extents: half }
                    .to_rapier_collider_with_material(PhysicsMaterial {
                        friction: 0.85,
                        restitution: 0.02,
                    });
                let (rb, col) = physics.add_dynamic_body(position, collider, mass);
                // Spawn frozen: the block behaves as level geometry until an
                // impact or lost support wakes it (structure_support system).
                physics.freeze_body(rb);

                let entity = world.spawn();
                world.add_component(entity, Transform::new(position, Quat::IDENTITY, half * 2.0));
                world.add_component(entity, RenderMesh { mesh, material });
                world.add_component(entity, PhysicsBody::new(rb, col, false));
                world.add_component(
                    entity,
                    crate::game::DestructibleBlock {
                        half_height: half.y,
                    },
                );
                physics.register_entity(col, entity);
                spawned += 1;
            }
        }
    }
    log::info!("Spawned destructible structure with {spawned} blocks");
}

fn finite_clamped(value: f32, fallback: f32, minimum: f32, maximum: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

fn finite_vec3(values: [f32; 3], fallback: Vec3, limit: f32) -> Vec3 {
    let parsed = Vec3::from_array(values);
    if parsed.is_finite() {
        parsed.clamp(Vec3::splat(-limit), Vec3::splat(limit))
    } else {
        fallback
    }
}

fn sanitize_player(desc: &PlayerDesc) -> PlayerDesc {
    // Fallbacks reuse the serde defaults so the two never drift apart.
    PlayerDesc {
        position: finite_vec3(
            desc.position,
            Vec3::from_array(super::default_player_position()),
            100_000.0,
        )
        .to_array(),
        height: finite_clamped(desc.height, super::default_player_height(), 0.8, 3.0),
        move_speed: finite_clamped(desc.move_speed, super::default_move_speed(), 0.5, 30.0),
        mouse_sensitivity: finite_clamped(
            desc.mouse_sensitivity,
            super::default_mouse_sensitivity(),
            0.0001,
            0.02,
        ),
    }
}

fn build_mesh(source: &MeshSource) -> Mesh {
    match source {
        MeshSource::Cube => ProceduralGenerator::create_cube(),
        MeshSource::Sphere { segments, rings } => {
            ProceduralGenerator::create_sphere(*segments, *rings)
        }
        MeshSource::Cylinder { segments } => ProceduralGenerator::create_cylinder(*segments),
        MeshSource::Plane { size, subdivisions } => {
            ProceduralGenerator::create_plane(*size, *subdivisions)
        }
        MeshSource::Rifle => ProceduralGenerator::create_rifle(),
        MeshSource::Wedge => ProceduralGenerator::create_wedge(),
        MeshSource::Humanoid => ProceduralGenerator::create_humanoid(),
        MeshSource::Gltf { .. } => unreachable!("glTF meshes are loaded directly into assets"),
    }
}

fn build_material(desc: &MaterialDesc) -> Material {
    Material {
        name: desc.name.clone(),
        albedo_factor: [
            finite_clamped(desc.color[0], 0.5, 0.0, 1.0),
            finite_clamped(desc.color[1], 0.5, 0.0, 1.0),
            finite_clamped(desc.color[2], 0.5, 0.0, 1.0),
            finite_clamped(desc.alpha, 1.0, 0.0, 1.0),
        ],
        metallic: finite_clamped(desc.metallic, 0.0, 0.0, 1.0),
        roughness: finite_clamped(desc.roughness, 0.5, 0.02, 1.0),
        emissive_factor: desc
            .emissive
            .map(|value| finite_clamped(value, 0.0, 0.0, 100.0)),
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
        water: desc.water,
    }
}

fn build_light(desc: &LightDesc) -> Light {
    match desc {
        LightDesc::Directional {
            direction,
            color,
            intensity,
        } => {
            let direction = finite_vec3(*direction, Vec3::new(0.4, -1.0, 0.2), 1_000.0);
            let direction = if direction.length_squared() > 1.0e-8 {
                direction.normalize()
            } else {
                Vec3::NEG_Y
            };
            Light::directional(
                direction,
                color.map(|value| finite_clamped(value, 1.0, 0.0, 10.0)),
                finite_clamped(*intensity, 1.0, 0.0, 100_000.0),
            )
        }
        LightDesc::Point {
            position,
            color,
            intensity,
            range,
        } => Light::point(
            finite_vec3(*position, Vec3::ZERO, 100_000.0),
            color.map(|value| finite_clamped(value, 1.0, 0.0, 10.0)),
            finite_clamped(*intensity, 1.0, 0.0, 100_000.0),
            finite_clamped(*range, 10.0, 0.1, 100_000.0),
        ),
        LightDesc::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_angle,
            outer_angle,
        } => {
            let inner_angle = finite_clamped(
                *inner_angle,
                20.0_f32.to_radians(),
                0.0,
                std::f32::consts::FRAC_PI_2,
            );
            let outer_angle = finite_clamped(
                *outer_angle,
                35.0_f32.to_radians(),
                inner_angle,
                std::f32::consts::FRAC_PI_2,
            );
            Light::spot(
                finite_vec3(*position, Vec3::ZERO, 100_000.0),
                finite_vec3(*direction, Vec3::NEG_Y, 1_000.0).normalize_or_zero(),
                color.map(|value| finite_clamped(value, 1.0, 0.0, 10.0)),
                finite_clamped(*intensity, 1.0, 0.0, 100_000.0),
                finite_clamped(*range, 10.0, 0.1, 100_000.0),
                inner_angle,
                outer_angle,
            )
        }
    }
}

fn build_transform(desc: &TransformDesc) -> Transform {
    let [rx, ry, rz] = desc
        .rotation
        .map(|angle| finite_clamped(angle, 0.0, -360_000.0, 360_000.0));
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        rx.to_radians(),
        ry.to_radians(),
        rz.to_radians(),
    );
    let scale = desc.scale.map(|axis| {
        let axis = if axis.is_finite() { axis } else { 1.0 };
        let sign = if axis.is_sign_negative() { -1.0 } else { 1.0 };
        sign * axis.abs().clamp(0.001, 10_000.0)
    });
    Transform::new(
        finite_vec3(desc.position, Vec3::ZERO, 100_000.0),
        rotation,
        Vec3::from_array(scale),
    )
}

fn build_shape(desc: &ShapeDesc) -> PhysicsShape {
    match desc {
        ShapeDesc::Sphere(radius) => PhysicsShape::Sphere { radius: *radius },
        ShapeDesc::Cuboid(half_extents) => PhysicsShape::Cuboid {
            half_extents: Vec3::from_array(*half_extents),
        },
        ShapeDesc::Capsule([radius, half_height]) => PhysicsShape::Capsule {
            radius: *radius,
            half_height: *half_height,
        },
        ShapeDesc::Cylinder([radius, half_height]) => PhysicsShape::Cylinder {
            radius: *radius,
            half_height: *half_height,
        },
        ShapeDesc::Wedge(half_extents) => PhysicsShape::Wedge {
            half_extents: Vec3::from_array(*half_extents),
        },
    }
}

fn surface_material(desc: &SurfaceDesc) -> PhysicsMaterial {
    match desc {
        SurfaceDesc::Preset(preset) => match preset {
            SurfacePreset::Default => PhysicsMaterial::DEFAULT,
            SurfacePreset::Ice => PhysicsMaterial::ICE,
            SurfacePreset::Rubber => PhysicsMaterial::RUBBER,
            SurfacePreset::Metal => PhysicsMaterial::METAL,
        },
        SurfaceDesc::Custom {
            friction,
            restitution,
        } => PhysicsMaterial {
            friction: *friction,
            restitution: *restitution,
        },
    }
}

fn spawn_entity(
    desc: &EntityDesc,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
    meshes: &HashMap<String, Handle<Mesh>>,
    materials: &HashMap<String, Handle<Material>>,
    gltf_materials: &HashMap<String, Handle<Material>>,
) {
    let Some(mesh) = meshes.get(&desc.mesh).copied() else {
        log::warn!(
            "Skipping entity: unknown mesh `{}` (declare it in `meshes`)",
            desc.mesh
        );
        return;
    };
    let material = if desc.material == "$gltf" {
        gltf_materials.get(&desc.mesh).copied()
    } else {
        materials.get(&desc.material).copied()
    };
    let Some(material) = material else {
        log::warn!(
            "Skipping entity: unknown material `{}` for mesh `{}`",
            desc.material,
            desc.mesh
        );
        return;
    };

    let transform = build_transform(&desc.transform);
    let entity = world.spawn();
    world.add_component(entity, transform);
    world.add_component(entity, RenderMesh { mesh, material });

    if let Some(name) = &desc.name {
        // Scene loading is a one-time startup step, so leaking the name to get
        // the `&'static str` the `Name` component expects is bounded and fine.
        let leaked: &'static str = Box::leak(name.clone().into_boxed_str());
        world.add_component(entity, Name(leaked));
    }

    if let Some(physics_desc) = &desc.physics {
        let is_door = matches!(
            desc.interaction.as_ref(),
            Some(super::InteractionDesc::Door { .. })
        );
        attach_physics(entity, physics_desc, &transform, is_door, world, physics);
    }
    if let Some(super::InteractionDesc::Door {
        open_rotation,
        hinge_offset,
        speed,
    }) = &desc.interaction
    {
        let [rx, ry, rz] =
            open_rotation.map(|angle| finite_clamped(angle, 0.0, -360_000.0, 360_000.0));
        let relative = Quat::from_euler(
            EulerRot::XYZ,
            rx.to_radians(),
            ry.to_radians(),
            rz.to_radians(),
        );
        world.add_component(
            entity,
            ToggleDoor {
                closed_position: transform.position,
                closed_rotation: transform.rotation,
                open_rotation: transform.rotation * relative,
                hinge_offset: finite_vec3(*hinge_offset, Vec3::ZERO, 10_000.0),
                open: false,
                progress: 0.0,
                speed: speed.clamp(0.1, 20.0),
            },
        );
    }
    if let Some(super::InteractionDesc::Explosive { radius, impulse }) = &desc.interaction {
        world.add_component(
            entity,
            Explosive {
                radius: finite_clamped(*radius, super::default_explosion_radius(), 1.0, 20.0),
                impulse: finite_clamped(*impulse, super::default_explosion_impulse(), 1.0, 80.0),
            },
        );
    }
}

fn attach_physics(
    entity: Entity,
    desc: &PhysicsDesc,
    transform: &Transform,
    kinematic: bool,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
) {
    let shape = build_shape(&desc.shape);
    let collider = shape.to_rapier_collider_with_material(surface_material(&desc.surface));
    let mass = if desc.mass.is_finite() {
        desc.mass.clamp(0.0, 100_000.0)
    } else {
        0.0
    };
    let is_static = mass <= 0.0;
    let (rb, col) = if kinematic {
        physics.add_kinematic_body(transform.position, collider)
    } else if is_static {
        physics.add_static_body(transform.position, collider)
    } else {
        physics.add_dynamic_body(transform.position, collider, mass)
    };
    physics.set_body_rotation(rb, transform.rotation);
    world.add_component(entity, PhysicsBody::new(rb, col, is_static || kinematic));
    physics.register_entity(col, entity);

    let impulse = finite_vec3(desc.initial_impulse, Vec3::ZERO, 10_000.0);
    if impulse.length_squared() > 0.0 {
        physics.apply_impulse(rb, impulse);
    }
}

fn spawn_enemies(
    desc: &EnemyDesc,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
    meshes: &HashMap<String, Handle<Mesh>>,
    materials: &HashMap<String, Handle<Material>>,
) -> Vec<Entity> {
    let mut spawner = EnemySpawner::new();
    spawner.spawn_points = desc
        .spawn_points
        .iter()
        .copied()
        .map(Vec3::from_array)
        .filter(|position| position.is_finite())
        .collect();
    spawner.waypoints = desc
        .waypoints
        .iter()
        .copied()
        .map(Vec3::from_array)
        .filter(|position| position.is_finite())
        .collect();
    spawner.patrol_routes = desc
        .routes
        .iter()
        .map(|route| {
            route
                .iter()
                .copied()
                .map(Vec3::from_array)
                .filter(|position| position.is_finite())
                .collect()
        })
        .collect();
    spawner.enemy_mesh = meshes.get(&desc.mesh).copied();
    spawner.enemy_material = materials.get(&desc.material).copied();
    if spawner.enemy_mesh.is_none() {
        log::warn!("Enemy mesh `{}` not found in scene meshes", desc.mesh);
    }
    if spawner.enemy_material.is_none() {
        log::warn!(
            "Enemy material `{}` not found in scene materials",
            desc.material
        );
    }
    spawner.spawn_enemies(world, physics)
}

#[cfg(test)]
mod tests {
    use super::{build_light, build_material, build_transform, sanitize_player};
    use crate::game::scene::{LightDesc, MaterialDesc, PlayerDesc, TransformDesc};

    #[test]
    fn invalid_authored_values_are_sanitized_before_runtime() {
        let player = sanitize_player(&PlayerDesc {
            position: [f32::NAN, 2.0, 3.0],
            height: f32::NEG_INFINITY,
            move_speed: -4.0,
            mouse_sensitivity: f32::NAN,
        });
        assert!(player.position.into_iter().all(f32::is_finite));
        assert!((0.8..=3.0).contains(&player.height));
        assert!(player.move_speed > 0.0);
        assert!(player.mouse_sensitivity > 0.0);

        let transform = build_transform(&TransformDesc {
            position: [f32::INFINITY, 0.0, 0.0],
            rotation: [0.0, f32::NAN, 0.0],
            scale: [0.0, f32::NAN, -f32::INFINITY],
        });
        assert!(transform.position.is_finite());
        assert!(transform.rotation.is_finite());
        assert!(transform.scale.is_finite());
        assert!(transform.scale.abs().min_element() >= 0.001);

        let material = build_material(&MaterialDesc {
            name: "invalid".to_string(),
            color: [f32::NAN, -2.0, 9.0],
            alpha: f32::NAN,
            roughness: f32::INFINITY,
            metallic: -4.0,
            emissive: [f32::NAN, -1.0, 999.0],
            water: false,
        });
        assert!(material.albedo_factor.iter().all(|value| value.is_finite()));
        assert!((0.0..=1.0).contains(&material.metallic));
        assert!((0.02..=1.0).contains(&material.roughness));
        assert!(material
            .emissive_factor
            .iter()
            .all(|value| value.is_finite()));

        let light = build_light(&LightDesc::Directional {
            direction: [0.0, 0.0, 0.0],
            color: [f32::NAN, 1.0, 1.0],
            intensity: f32::NAN,
        });
        assert!(light.position.is_finite());
        assert!(light.color.iter().all(|value| value.is_finite()));
        assert!(light.intensity.is_finite());
    }
}
