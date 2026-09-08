//! Fixed-tick drivers for the physics showcase effects (tornado, water
//! buoyancy, destructible structures) and the per-frame water mesh animation.

use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::{DestructibleBlock, Tornado, TornadoDust, WaterSurface};
use crate::physics::{PhysicsBody, PhysicsWorld, Ray};
use glam::{Vec2, Vec3};

/// Density of the fake water in kg/m^3 relative to body volume estimated
/// from collider AABBs. Slightly under real water so heavy metal props sink.
const WATER_DENSITY: f32 = 780.0;
/// Linear drag applied to submerged bodies (water is thick).
const WATER_LINEAR_DRAG: f32 = 2.4;
const WATER_ANGULAR_DRAG: f32 = 1.6;
/// Impulse magnitude above which a frozen structure block breaks loose.
const BLOCK_BREAK_IMPULSE: f32 = 9.0;

/// FixedUpdate: apply tornado forces, water buoyancy, and structure support
/// checks at the physics cadence.
pub fn fixed_update(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.fixed_timestep)
        .unwrap_or(0.0);
    if dt <= 0.0 {
        return;
    }

    tornado_forces(world, resources, dt);
    water_buoyancy(world, resources);
    structure_support(world, resources);
}

/// Update per-effect clocks with *scaled* time so pausing the simulation
/// also freezes the tornado wander and the wave phase that drives buoyancy.
pub fn update(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(0.0);
    for tornado in world.ecs.query::<&mut Tornado>().iter() {
        tornado.time += dt;
    }
    if let Some(mut water) = resources.get_mut::<Option<WaterSurface>>() {
        if let Some(water) = water.as_mut() {
            water.time += dt;
        }
    }

    animate_water_mesh(resources);
    animate_tornado_dust(world, resources);
}

fn tornado_forces(world: &mut EngineWorld, resources: &Resources, _dt: f32) {
    let tornadoes: Vec<Tornado> = world.ecs.query::<&Tornado>().iter().cloned().collect();
    if tornadoes.is_empty() {
        return;
    }

    let mut physics = resources.expect_mut::<PhysicsWorld>();
    for tornado in &tornadoes {
        let base = tornado.axis_base();
        for (_, body) in physics.rigid_body_set.iter_mut() {
            if !body.is_dynamic() {
                continue;
            }
            let position = Vec3::new(
                body.translation().x,
                body.translation().y,
                body.translation().z,
            );
            let relative_height = position.y - base.y;
            if relative_height < -1.0 || relative_height > tornado.height {
                continue;
            }
            let to_axis = Vec2::new(position.x - base.x, position.z - base.z);
            let distance = to_axis.length();
            if distance > tornado.radius {
                continue;
            }

            // The funnel widens with height; force peaks at the core wall
            // and fades toward the rim and the top.
            let core_radius = 1.2 + (relative_height / tornado.height) * tornado.radius * 0.45;
            let radial_falloff = 1.0 - (distance / tornado.radius);
            let height_falloff = 1.0 - (relative_height / tornado.height) * 0.6;
            let force_scale = radial_falloff * height_falloff * tornado.strength;

            let radial_dir = if distance > 0.05 {
                to_axis / distance
            } else {
                Vec2::X
            };
            // Tangential swirl (counter-clockwise seen from above).
            let tangent = Vec2::new(-radial_dir.y, radial_dir.x);
            // Inside the core wall the flow pushes outward (bodies get flung);
            // outside it gets sucked inward.
            let radial_strength = if distance < core_radius { 0.35 } else { -0.55 };
            let lift = 1.15;

            let force = Vec3::new(
                tangent.x * force_scale + radial_dir.x * force_scale * radial_strength,
                lift * force_scale * radial_falloff,
                tangent.y * force_scale + radial_dir.y * force_scale * radial_strength,
            );
            let mass_scale = body.mass().clamp(0.2, 60.0);
            body.wake_up(true);
            body.add_force(
                rapier3d::math::Vector::new(force.x, force.y, force.z)
                    * (mass_scale / 10.0).min(2.5),
                true,
            );
        }
    }
}

fn water_buoyancy(world: &mut EngineWorld, resources: &Resources) {
    let Some(water_guard) = resources.get::<Option<WaterSurface>>() else {
        return;
    };
    let Some(water) = water_guard.as_ref() else {
        return;
    };
    let half = water.size * 0.5;
    let level = water.center.y;
    let (time, amplitude) = (water.time, water.amplitude);
    let (center_x, center_z) = (water.center.x, water.center.z);
    drop(water_guard);

    // Skip kinematic actors (player/enemies): the character controller owns
    // their vertical motion, buoyancy would fight it.
    let dynamic_entities: Vec<rapier3d::prelude::RigidBodyHandle> = world
        .ecs
        .query::<&PhysicsBody>()
        .iter()
        .map(|body| body.rigid_body_handle)
        .collect();

    let mut physics = resources.expect_mut::<PhysicsWorld>();
    for handle in dynamic_entities {
        let Some(collider_handle) = physics
            .rigid_body_set
            .get(handle)
            .filter(|body| body.is_dynamic())
            .and_then(|body| body.colliders().first().copied())
        else {
            continue;
        };
        let Some(collider) = physics.collider_set.get(collider_handle) else {
            continue;
        };
        let aabb = collider.compute_aabb();
        let volume = ((aabb.maxs.x - aabb.mins.x)
            * (aabb.maxs.y - aabb.mins.y)
            * (aabb.maxs.z - aabb.mins.z))
            .max(0.001);
        let (bottom, top) = (aabb.mins.y, aabb.maxs.y);

        let Some(body) = physics.rigid_body_set.get_mut(handle) else {
            continue;
        };
        let position = body.translation();
        if (position.x - center_x).abs() > half || (position.z - center_z).abs() > half {
            continue;
        }

        let surface = level + crate::game::wave_height(position.x, position.z, time, amplitude);
        if bottom >= surface {
            continue;
        }
        let submerged_fraction = ((surface - bottom) / (top - bottom).max(0.01)).clamp(0.0, 1.0);

        // Archimedes with the AABB volume as displaced volume. `add_force`
        // wants newtons; gravity is applied by the pipeline, so this force
        // exceeding m*g is exactly what makes light bodies bob up.
        let buoyancy = WATER_DENSITY * volume * submerged_fraction * 9.81;
        // Wave slope pushes floaters along the wave travel direction.
        let gradient = crate::game::wave_gradient(position.x, position.z, time, amplitude);
        let mass = body.mass();
        let drag = submerged_fraction * WATER_LINEAR_DRAG * mass;
        let linvel = body.linvel();

        body.wake_up(true);
        body.add_force(
            rapier3d::math::Vector::new(
                -gradient.x * buoyancy * 0.12 - linvel.x * drag,
                buoyancy - linvel.y * drag,
                -gradient.y * buoyancy * 0.12 - linvel.z * drag,
            ),
            true,
        );
        let angvel = body.angvel();
        body.set_angvel(
            angvel * (1.0 - (submerged_fraction * WATER_ANGULAR_DRAG * 0.02)).max(0.0),
            true,
        );
    }
}

/// Unfreeze structure blocks that lost support: a frozen block stays frozen
/// only while something solid (ground or another block) sits directly below
/// it. Blocks knocked hard enough are unfrozen by the weapon/explosion code
/// via [`try_break_block`].
fn structure_support(world: &mut EngineWorld, resources: &Resources) {
    let blocks: Vec<(hecs::Entity, Vec3, f32, rapier3d::prelude::RigidBodyHandle)> = world
        .ecs
        .query::<(hecs::Entity, &DestructibleBlock, &Transform, &PhysicsBody)>()
        .iter()
        .map(|(entity, block, transform, body)| {
            (
                entity,
                transform.position,
                block.half_height,
                body.rigid_body_handle,
            )
        })
        .collect();
    if blocks.is_empty() {
        return;
    }

    let mut physics = resources.expect_mut::<PhysicsWorld>();
    let mut to_unfreeze = Vec::new();
    for (_entity, position, half_height, handle) in &blocks {
        if !physics.body_is_frozen(*handle) {
            continue;
        }
        // Cast slightly past the block's bottom face; anything solid there
        // counts as support.
        let ray = Ray::new(*position, Vec3::NEG_Y, half_height + 0.35);
        let supported = physics.cast_ray_excluding_body(&ray, *handle).is_some();
        if !supported {
            to_unfreeze.push(*handle);
        }
    }
    for handle in to_unfreeze {
        physics.unfreeze_body(handle);
    }
}

/// Called from the weapon/explosion code when a frozen destructible block
/// receives an impulse. Unfreezes it if the hit is hard enough, then applies
/// the impulse so the block actually flies.
pub fn try_break_block(
    world: &EngineWorld,
    physics: &mut PhysicsWorld,
    entity: hecs::Entity,
    impulse: Vec3,
    point: Vec3,
) -> bool {
    if world.ecs.get::<&DestructibleBlock>(entity).is_err() {
        return false;
    }
    let Ok(body) = world.ecs.get::<&PhysicsBody>(entity) else {
        return false;
    };
    let handle = body.rigid_body_handle;
    drop(body);
    if impulse.length() < BLOCK_BREAK_IMPULSE && physics.body_is_frozen(handle) {
        return true; // Absorbed: too weak to break the block loose.
    }
    physics.unfreeze_body(handle);
    physics.apply_impulse_at_point(handle, impulse, point);
    true
}

/// Rebuild the water surface mesh vertices from the wave function and
/// re-upload the GPU vertex buffer. ~9k vertices at 96x96; one memcpy-sized
/// write per frame, same order of magnitude as the object uniform upload.
fn animate_water_mesh(resources: &Resources) {
    let Some(water_guard) = resources.get::<Option<WaterSurface>>() else {
        return;
    };
    let Some(water) = water_guard.as_ref() else {
        return;
    };
    let mesh_handle = water.mesh;
    let (time, amplitude) = (water.time, water.amplitude);
    let base = water.base_vertices.clone();
    drop(water_guard);

    let mut assets = resources.expect_mut::<crate::asset::AssetManager>();
    let Some(mesh) = assets.meshes.get_mut(mesh_handle) else {
        return;
    };
    mesh.vertices = base;
    for vertex in &mut mesh.vertices {
        let [x, _, z] = vertex.position;
        vertex.position[1] += crate::game::wave_height(x, z, time, amplitude);
        let gradient = crate::game::wave_gradient(x, z, time, amplitude);
        let normal = Vec3::new(-gradient.x, 1.0, -gradient.y).normalize();
        vertex.normal = normal.into();
    }

    let renderer = resources.expect::<crate::renderer::Renderer>();
    if let Some(buffer) = &mesh.vertex_buffer {
        renderer
            .queue
            .write_buffer(buffer, 0, bytemuck::cast_slice(&mesh.vertices));
    }
}

/// Swirl the visual dust motes around each tornado funnel.
fn animate_tornado_dust(world: &mut EngineWorld, _resources: &Resources) {
    let tornadoes: Vec<Tornado> = world.ecs.query::<&Tornado>().iter().cloned().collect();
    let Some(tornado) = tornadoes.first() else {
        return;
    };
    let base = tornado.axis_base();
    for (dust, transform) in world.ecs.query::<(&TornadoDust, &mut Transform)>().iter() {
        // Height cycles upward over time; radius follows the funnel cone.
        let cycle = (tornado.time * (0.16 + dust.seed * 0.1) + dust.seed * 7.0).fract();
        let height = cycle * tornado.height;
        let funnel_radius = 1.4 + (height / tornado.height) * tornado.radius * 0.5;
        let angle =
            tornado.time * (2.2 + dust.seed * 1.6) + dust.seed * std::f32::consts::TAU * 3.0;
        transform.position = Vec3::new(
            base.x + angle.cos() * funnel_radius,
            base.y + height,
            base.z + angle.sin() * funnel_radius,
        );
        let size = 0.14 + dust.seed * 0.2 + (height / tornado.height) * 0.16;
        transform.scale = Vec3::splat(size);
    }
}
