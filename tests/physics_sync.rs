use glam::{Quat, Vec3};
use gxengine::core::{EngineWorld, Frozen, Resources, Transform};
use gxengine::game::systems::physics_sync::step_physics;
use gxengine::physics::{PhysicsBody, PhysicsShape, PhysicsWorld};

#[test]
fn dynamic_body_falls_under_gravity() {
    let mut world = EngineWorld::new();
    let resources = Resources::new();
    let mut physics = PhysicsWorld::default();

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, col) =
        physics.add_dynamic_body(Vec3::new(0.0, 10.0, 0.0), shape.to_rapier_collider(), 1.0);

    let entity = world.spawn();
    world.add_component(
        entity,
        Transform::new(Vec3::new(0.0, 10.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    world.add_component(entity, PhysicsBody::new(rb, col, false));

    resources.insert(physics);

    for _ in 0..60 {
        step_physics(&mut world, &resources);
    }

    let transform = world.get_component::<Transform>(entity).unwrap();
    assert!(transform.position.y < 9.0);
}

#[test]
fn frozen_body_does_not_fall() {
    let mut world = EngineWorld::new();
    let resources = Resources::new();
    let mut physics = PhysicsWorld::default();

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, col) =
        physics.add_dynamic_body(Vec3::new(0.0, 10.0, 0.0), shape.to_rapier_collider(), 1.0);

    let entity = world.spawn();
    world.add_component(
        entity,
        Transform::new(Vec3::new(0.0, 10.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    world.add_component(entity, Frozen);
    world.add_component(entity, PhysicsBody::new(rb, col, false));

    resources.insert(physics);

    for _ in 0..60 {
        step_physics(&mut world, &resources);
    }

    let transform = world.get_component::<Transform>(entity).unwrap();
    assert!((transform.position.y - 10.0).abs() < 0.01);
    let physics = resources.get::<PhysicsWorld>().unwrap();
    let body = physics.rigid_body_set.get(rb).unwrap();
    assert!(body.is_fixed());
    assert!((body.translation().y - 10.0).abs() < 0.01);
}
