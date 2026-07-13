use crate::core::{EngineWorld, Frozen, Resources, Transform};
use crate::physics::{PhysicsBody, PhysicsWorld};

pub fn step_physics(world: &mut EngineWorld, resources: &Resources) {
    let mut physics = resources.expect_mut::<PhysicsWorld>();

    sync_to_physics(world, &mut physics);

    physics.step();

    sync_from_physics(world, &physics);
}

fn sync_to_physics(world: &mut EngineWorld, physics: &mut PhysicsWorld) {
    for (transform, body, frozen) in world
        .ecs
        .query::<(&Transform, &PhysicsBody, Option<&Frozen>)>()
        .iter()
    {
        if frozen.is_some() {
            if let Some(rb) = physics.rigid_body_set.get_mut(body.rigid_body_handle) {
                rb.set_body_type(rapier3d::prelude::RigidBodyType::Fixed, true);
                rb.set_translation(transform.position, true);
                rb.set_rotation(transform.rotation, true);
                rb.set_linvel(glam::Vec3::ZERO, true);
                rb.set_angvel(glam::Vec3::ZERO, true);
            }
            continue;
        }
        let is_kinematic = physics
            .rigid_body_set
            .get(body.rigid_body_handle)
            .map(|rb| rb.is_kinematic())
            .unwrap_or(false);
        if body.is_static || !is_kinematic {
            continue;
        }
        if let Some(rigid_body) = physics.rigid_body_set.get_mut(body.rigid_body_handle) {
            rigid_body.set_next_kinematic_translation(transform.position);
            rigid_body.set_next_kinematic_rotation(transform.rotation);
        }
    }
}

fn sync_from_physics(world: &mut EngineWorld, physics: &PhysicsWorld) {
    for (transform, body, frozen) in world
        .ecs
        .query::<(&mut Transform, &PhysicsBody, Option<&Frozen>)>()
        .iter()
    {
        let is_dynamic = physics
            .rigid_body_set
            .get(body.rigid_body_handle)
            .map(|rb| rb.is_dynamic())
            .unwrap_or(false);
        if body.is_static || !is_dynamic || frozen.is_some() {
            continue;
        }
        if let Some(pos) = physics.get_body_position(body.rigid_body_handle) {
            transform.position = pos;
        }
        if let Some(rot) = physics.get_body_rotation(body.rigid_body_handle) {
            transform.rotation = rot;
        }
    }
}
