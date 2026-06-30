use crate::core::{EngineWorld, Frozen, Resources, Transform};
use crate::physics::{PhysicsBody, PhysicsWorld};

pub fn step_physics(world: &mut EngineWorld, resources: &Resources) {
    let mut physics = resources
        .expect_mut::<PhysicsWorld>();

    sync_to_physics(world, &mut physics);

    physics.step();

    sync_from_physics(world, &physics);
}

fn sync_to_physics(world: &mut EngineWorld, physics: &mut PhysicsWorld) {
    for (transform, body, _frozen) in world
        .ecs
        .query::<(&Transform, &PhysicsBody, Option<&Frozen>)>()
        .iter()
    {
        if body.is_static || is_body_fixed(physics, body.rigid_body_handle) || _frozen.is_some() {
            continue;
        }
        physics.set_body_position(body.rigid_body_handle, transform.position);
        physics.set_body_rotation(body.rigid_body_handle, transform.rotation);
    }
}

fn sync_from_physics(world: &mut EngineWorld, physics: &PhysicsWorld) {
    for (transform, body, _frozen) in world
        .ecs
        .query::<(&mut Transform, &PhysicsBody, Option<&Frozen>)>()
        .iter()
    {
        if body.is_static || is_body_fixed(physics, body.rigid_body_handle) || _frozen.is_some() {
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

fn is_body_fixed(physics: &PhysicsWorld, handle: rapier3d::prelude::RigidBodyHandle) -> bool {
    physics
        .rigid_body_set
        .get(handle)
        .map(|rb| rb.is_fixed())
        .unwrap_or(false)
}
