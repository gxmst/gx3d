//! Ragdoll spawning: a chain of dynamic rigid bodies linked by spherical
//! joints — the DESIGN.md "物体行为" experiment (constraint-based dolls).
//!
//! Press K in any scene to drop one in front of the crosshair. Every part is
//! a normal sandbox-manipulable dynamic body: grab a limb with E, throw the
//! whole doll with T, freeze it mid-flight with X.

use crate::core::{EngineWorld, Resources, Transform};
use crate::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld};
use crate::renderer::{Camera, RenderMesh};
use glam::{Quat, Vec3};
use rapier3d::prelude::{RigidBodyHandle, SphericalJointBuilder};
use winit::keyboard::KeyCode;

/// Cap on simultaneously alive ragdolls; the oldest is removed beyond this.
const MAX_RAGDOLLS: usize = 6;

/// Marks one body part of a spawned ragdoll (`id` groups parts of one doll).
#[derive(Debug, Clone, Copy)]
pub struct RagdollPart {
    pub id: u64,
}

/// Running counter + spawn order for cleanup.
#[derive(Default)]
pub struct RagdollState {
    next_id: u64,
    spawned: Vec<u64>,
}

pub fn update(world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        return;
    }
    let buy_open = resources
        .get::<super::buy_menu::BuyState>()
        .map(|buy| buy.open)
        .unwrap_or(false);
    let pressed = resources
        .get::<crate::input::InputState>()
        .map(|input| input.is_key_just_pressed(KeyCode::KeyK))
        .unwrap_or(false);
    if !pressed || buy_open {
        return;
    }

    let (origin, forward) = {
        let camera = resources.expect::<Camera>();
        (camera.position, camera.forward())
    };
    let spawn_at = origin + forward * 3.0 + Vec3::Y * 0.4;
    spawn_ragdoll(world, resources, spawn_at);
}

/// Build one doll: pelvis, torso, head, two two-part arms, two two-part legs
/// (11 bodies, 10 spherical joints) linked with tight anchor points so it
/// tumbles like a jointed figure instead of a soup of parts.
pub fn spawn_ragdoll(world: &mut EngineWorld, resources: &Resources, at: Vec3) {
    let (cube_mesh, sphere_mesh, skin, cloth) = {
        let Some(sandbox) = resources.get::<crate::physics::PhysicsSandbox>() else {
            return;
        };
        (
            sandbox.cube_mesh,
            sandbox.sphere_mesh,
            sandbox.heavy_material,
            sandbox.box_material,
        )
    };

    let id = {
        let mut state = resources.expect_mut::<super::ragdoll::RagdollState>();
        state.next_id += 1;
        let id = state.next_id;
        state.spawned.push(id);
        id
    };

    let mut physics = resources.expect_mut::<PhysicsWorld>();

    // Part builder: box part with given half extents at an offset from `at`.
    let mut parts: Vec<(RigidBodyHandle, hecs::Entity)> = Vec::new();
    let spawn_part = |world: &mut EngineWorld,
                      physics: &mut PhysicsWorld,
                      offset: Vec3,
                      half: Vec3,
                      mass: f32,
                      round: bool|
     -> (RigidBodyHandle, hecs::Entity) {
        let position = at + offset;
        let shape = if round {
            PhysicsShape::Sphere { radius: half.x }
        } else {
            PhysicsShape::Cuboid { half_extents: half }
        };
        let collider = shape.to_rapier_collider_with_material(PhysicsMaterial {
            friction: 0.8,
            restitution: 0.05,
        });
        let (rb, col) = physics.add_dynamic_body(position, collider, mass);
        let entity = world.spawn();
        world.add_component(
            entity,
            Transform::new(
                position,
                Quat::IDENTITY,
                if round {
                    Vec3::splat(half.x * 2.0)
                } else {
                    half * 2.0
                },
            ),
        );
        world.add_component(
            entity,
            RenderMesh {
                mesh: if round { sphere_mesh } else { cube_mesh },
                material: if round { skin } else { cloth },
            },
        );
        world.add_component(entity, PhysicsBody::new(rb, col, false));
        world.add_component(entity, RagdollPart { id });
        physics.register_entity(col, entity);
        (rb, entity)
    };

    // Layout (local offsets, meters). Y up; doll spawns upright then flops.
    let pelvis = spawn_part(
        world,
        &mut physics,
        Vec3::ZERO,
        Vec3::new(0.16, 0.10, 0.10),
        8.0,
        false,
    );
    let torso = spawn_part(
        world,
        &mut physics,
        Vec3::Y * 0.32,
        Vec3::new(0.17, 0.16, 0.10),
        12.0,
        false,
    );
    let head = spawn_part(
        world,
        &mut physics,
        Vec3::Y * 0.62,
        Vec3::splat(0.11),
        4.0,
        true,
    );
    let arm_upper_l = spawn_part(
        world,
        &mut physics,
        Vec3::new(-0.30, 0.42, 0.0),
        Vec3::new(0.11, 0.05, 0.05),
        2.5,
        false,
    );
    let arm_lower_l = spawn_part(
        world,
        &mut physics,
        Vec3::new(-0.52, 0.42, 0.0),
        Vec3::new(0.10, 0.045, 0.045),
        1.8,
        false,
    );
    let arm_upper_r = spawn_part(
        world,
        &mut physics,
        Vec3::new(0.30, 0.42, 0.0),
        Vec3::new(0.11, 0.05, 0.05),
        2.5,
        false,
    );
    let arm_lower_r = spawn_part(
        world,
        &mut physics,
        Vec3::new(0.52, 0.42, 0.0),
        Vec3::new(0.10, 0.045, 0.045),
        1.8,
        false,
    );
    let leg_upper_l = spawn_part(
        world,
        &mut physics,
        Vec3::new(-0.09, -0.26, 0.0),
        Vec3::new(0.06, 0.13, 0.06),
        5.0,
        false,
    );
    let leg_lower_l = spawn_part(
        world,
        &mut physics,
        Vec3::new(-0.09, -0.52, 0.0),
        Vec3::new(0.055, 0.12, 0.055),
        3.5,
        false,
    );
    let leg_upper_r = spawn_part(
        world,
        &mut physics,
        Vec3::new(0.09, -0.26, 0.0),
        Vec3::new(0.06, 0.13, 0.06),
        5.0,
        false,
    );
    let leg_lower_r = spawn_part(
        world,
        &mut physics,
        Vec3::new(0.09, -0.52, 0.0),
        Vec3::new(0.055, 0.12, 0.055),
        3.5,
        false,
    );
    parts.extend([
        pelvis,
        torso,
        head,
        arm_upper_l,
        arm_lower_l,
        arm_upper_r,
        arm_lower_r,
        leg_upper_l,
        leg_lower_l,
        leg_upper_r,
        leg_lower_r,
    ]);

    // Joints: (parent, child, anchor-on-parent, anchor-on-child), local space.
    let joints: [(RigidBodyHandle, RigidBodyHandle, Vec3, Vec3); 10] = [
        (pelvis.0, torso.0, Vec3::Y * 0.12, -Vec3::Y * 0.18),
        (torso.0, head.0, Vec3::Y * 0.18, -Vec3::Y * 0.13),
        (
            torso.0,
            arm_upper_l.0,
            Vec3::new(-0.19, 0.10, 0.0),
            Vec3::new(0.12, 0.0, 0.0),
        ),
        (
            arm_upper_l.0,
            arm_lower_l.0,
            Vec3::new(-0.12, 0.0, 0.0),
            Vec3::new(0.11, 0.0, 0.0),
        ),
        (
            torso.0,
            arm_upper_r.0,
            Vec3::new(0.19, 0.10, 0.0),
            Vec3::new(-0.12, 0.0, 0.0),
        ),
        (
            arm_upper_r.0,
            arm_lower_r.0,
            Vec3::new(0.12, 0.0, 0.0),
            Vec3::new(-0.11, 0.0, 0.0),
        ),
        (
            pelvis.0,
            leg_upper_l.0,
            Vec3::new(-0.09, -0.11, 0.0),
            Vec3::new(0.0, 0.14, 0.0),
        ),
        (
            leg_upper_l.0,
            leg_lower_l.0,
            Vec3::new(0.0, -0.14, 0.0),
            Vec3::new(0.0, 0.13, 0.0),
        ),
        (
            pelvis.0,
            leg_upper_r.0,
            Vec3::new(0.09, -0.11, 0.0),
            Vec3::new(0.0, 0.14, 0.0),
        ),
        (
            leg_upper_r.0,
            leg_lower_r.0,
            Vec3::new(0.0, -0.14, 0.0),
            Vec3::new(0.0, 0.13, 0.0),
        ),
    ];
    for (parent, child, anchor_parent, anchor_child) in joints {
        let joint = SphericalJointBuilder::new()
            .local_anchor1(rapier3d::math::Vector::new(
                anchor_parent.x,
                anchor_parent.y,
                anchor_parent.z,
            ))
            .local_anchor2(rapier3d::math::Vector::new(
                anchor_child.x,
                anchor_child.y,
                anchor_child.z,
            ))
            .build();
        physics.impulse_joint_set.insert(parent, child, joint, true);
    }
    drop(physics);

    // Enforce the ragdoll cap: despawn the oldest doll's parts wholesale.
    let overflow = {
        let mut state = resources.expect_mut::<super::ragdoll::RagdollState>();
        if state.spawned.len() > MAX_RAGDOLLS {
            Some(state.spawned.remove(0))
        } else {
            None
        }
    };
    if let Some(old_id) = overflow {
        despawn_ragdoll(world, resources, old_id);
    }

    if let Some(mut toast) = resources.get_mut::<super::Toast>() {
        toast.show("布娃娃已生成（E 抓取 · T 投掷 · F6 看冲量）", 2.0);
    }
}

/// Remove every part (and physics body) of one doll.
fn despawn_ragdoll(world: &mut EngineWorld, resources: &Resources, id: u64) {
    let parts: Vec<(hecs::Entity, rapier3d::prelude::RigidBodyHandle)> = world
        .ecs
        .query::<(hecs::Entity, &RagdollPart, &PhysicsBody)>()
        .iter()
        .filter(|(_, part, _)| part.id == id)
        .map(|(entity, _, body)| (entity, body.rigid_body_handle))
        .collect();
    let mut physics = resources.expect_mut::<PhysicsWorld>();
    for (entity, handle) in parts {
        physics.remove_body(handle);
        drop_entity(world, entity);
    }
}

fn drop_entity(world: &mut EngineWorld, entity: hecs::Entity) {
    world.despawn(entity);
}
