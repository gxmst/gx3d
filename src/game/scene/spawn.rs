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
use crate::game::{EnemySpawner, ToggleDoor};
use crate::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Light, RenderMesh};

use super::{
    EnemyDesc, EntityDesc, LightDesc, MaterialDesc, MeshSource, PhysicsDesc, PlayerDesc, Scene,
    ShapeDesc, SurfaceDesc, SurfacePreset, TransformDesc,
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

    // --- Enemies ----------------------------------------------------------
    let enemies = scene
        .enemies
        .as_ref()
        .map(|desc| spawn_enemies(desc, world, physics, &meshes, &materials))
        .unwrap_or_default();

    SpawnedScene {
        meshes,
        materials,
        lights,
        enemies,
        player: scene.player.clone(),
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
        albedo_factor: [desc.color[0], desc.color[1], desc.color[2], desc.alpha],
        metallic: desc.metallic,
        roughness: desc.roughness,
        emissive_factor: desc.emissive,
        albedo_map: None,
        normal_map: None,
        metallic_roughness_map: None,
        emissive_map: None,
    }
}

fn build_light(desc: &LightDesc) -> Light {
    match desc {
        LightDesc::Directional {
            direction,
            color,
            intensity,
        } => Light::directional(Vec3::from_array(*direction), *color, *intensity),
        LightDesc::Point {
            position,
            color,
            intensity,
            range,
        } => Light::point(Vec3::from_array(*position), *color, *intensity, *range),
        LightDesc::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_angle,
            outer_angle,
        } => Light::spot(
            Vec3::from_array(*position),
            Vec3::from_array(*direction),
            *color,
            *intensity,
            *range,
            *inner_angle,
            *outer_angle,
        ),
    }
}

fn build_transform(desc: &TransformDesc) -> Transform {
    let [rx, ry, rz] = desc.rotation;
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        rx.to_radians(),
        ry.to_radians(),
        rz.to_radians(),
    );
    Transform::new(
        Vec3::from_array(desc.position),
        rotation,
        Vec3::from_array(desc.scale),
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
        attach_physics(entity, physics_desc, &transform, world, physics);
    }
    if let Some(super::InteractionDesc::Door {
        open_rotation,
        speed,
    }) = &desc.interaction
    {
        let [rx, ry, rz] = *open_rotation;
        let relative = Quat::from_euler(
            EulerRot::XYZ,
            rx.to_radians(),
            ry.to_radians(),
            rz.to_radians(),
        );
        world.add_component(
            entity,
            ToggleDoor {
                closed_rotation: transform.rotation,
                open_rotation: transform.rotation * relative,
                open: false,
                progress: 0.0,
                speed: *speed,
            },
        );
    }
}

fn attach_physics(
    entity: Entity,
    desc: &PhysicsDesc,
    transform: &Transform,
    world: &mut EngineWorld,
    physics: &mut PhysicsWorld,
) {
    let shape = build_shape(&desc.shape);
    let collider = shape.to_rapier_collider_with_material(surface_material(&desc.surface));
    let is_static = desc.mass <= 0.0;
    let (rb, col) = if is_static {
        physics.add_static_body(transform.position, collider)
    } else {
        physics.add_dynamic_body(transform.position, collider, desc.mass)
    };
    physics.set_body_rotation(rb, transform.rotation);
    world.add_component(entity, PhysicsBody::new(rb, col, is_static));
    physics.register_entity(col, entity);

    let impulse = Vec3::from_array(desc.initial_impulse);
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
        .collect();
    spawner.waypoints = desc
        .waypoints
        .iter()
        .copied()
        .map(Vec3::from_array)
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
