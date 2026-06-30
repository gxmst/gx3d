use glam::{Quat, Vec3};
use gxengine::core::{EngineWorld, Transform};
use gxengine::physics::sandbox::PhysicsSandbox;
use gxengine::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld, Ray};
use gxengine::renderer::RenderMesh;

#[test]
fn sandbox_spawns_prop_with_components() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let mut sandbox = PhysicsSandbox::default();

    let entity = sandbox.spawn_dynamic_box(
        &mut world,
        &mut physics,
        Default::default(),
        Default::default(),
        Transform::new(Vec3::new(0.0, 2.0, 0.0), Quat::IDENTITY, Vec3::ONE),
        0.5,
        1.0,
    );

    assert!(world.has_component::<Transform>(entity));
    assert!(world.has_component::<RenderMesh>(entity));
    assert!(world.has_component::<PhysicsBody>(entity));
}

#[test]
fn ray_pick_finds_spawned_body() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let sandbox = PhysicsSandbox::default();

    let ground_shape = PhysicsShape::Cuboid {
        half_extents: Vec3::new(10.0, 0.5, 10.0),
    };
    let (ground_rb, ground_col) =
        physics.add_static_body(Vec3::new(0.0, -0.5, 0.0), ground_shape.to_rapier_collider());
    let ground_entity = world.spawn();
    world.add_component(
        ground_entity,
        Transform::new(Vec3::new(0.0, -0.5, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    world.add_component(ground_entity, PhysicsBody::new(ground_rb, ground_col, true));
    physics.register_entity(ground_col, ground_entity);

    let box_shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (box_rb, box_col) = physics.add_dynamic_body(
        Vec3::new(0.0, 0.5, 0.0),
        box_shape.to_rapier_collider(),
        1.0,
    );
    let box_entity = world.spawn();
    world.add_component(
        box_entity,
        Transform::new(Vec3::new(0.0, 0.5, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    world.add_component(box_entity, PhysicsBody::new(box_rb, box_col, false));
    physics.register_entity(box_col, box_entity);

    physics.step();

    let ray = Ray::new(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 10.0);
    let picked = sandbox.ray_pick(&world, &physics, &ray);
    assert_eq!(picked, Some(box_rb));
}

#[test]
fn static_body_is_not_pickable() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let sandbox = PhysicsSandbox::default();

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, col) = physics.add_static_body(Vec3::ZERO, shape.to_rapier_collider());
    let entity = world.spawn();
    world.add_component(
        entity,
        Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE),
    );
    world.add_component(entity, PhysicsBody::new(rb, col, true));
    physics.register_entity(col, entity);
    physics.step();

    let ray = Ray::new(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 10.0);

    assert_eq!(sandbox.ray_pick(&world, &physics, &ray), None);
}

#[test]
fn physics_material_is_applied_to_collider() {
    let collider = PhysicsShape::Sphere { radius: 0.5 }
        .to_rapier_collider_with_material(PhysicsMaterial::RUBBER);

    assert!((collider.friction() - PhysicsMaterial::RUBBER.friction).abs() < f32::EPSILON);
    assert!((collider.restitution() - PhysicsMaterial::RUBBER.restitution).abs() < f32::EPSILON);
}

#[test]
fn throwing_held_body_applies_impulse_and_releases_it() {
    let mut physics = PhysicsWorld::default();
    let mut sandbox = PhysicsSandbox::default();

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, _col) =
        physics.add_dynamic_body(Vec3::new(0.0, 1.0, 0.0), shape.to_rapier_collider(), 1.0);

    sandbox.hold_body(rb, 3.0);
    assert!(sandbox.throw_held(&mut physics, Vec3::new(1.0, 0.0, 0.0), 8.0));

    assert!(sandbox.held_body.is_none());
    assert!(physics.get_body_velocity(rb).unwrap().x > 0.0);
}

#[test]
fn freeze_toggles_body_type() {
    let mut physics = PhysicsWorld::default();
    let sandbox = PhysicsSandbox::default();

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, _col) =
        physics.add_dynamic_body(Vec3::new(0.0, 1.0, 0.0), shape.to_rapier_collider(), 1.0);

    assert!(!sandbox.is_frozen(&physics, rb));
    sandbox.freeze(&mut physics, rb);
    assert!(sandbox.is_frozen(&physics, rb));
    sandbox.unfreeze(&mut physics, rb);
    assert!(!sandbox.is_frozen(&physics, rb));
}

#[test]
fn dynamic_bodies_enable_ccd() {
    let mut physics = PhysicsWorld::default();
    let shape = PhysicsShape::Sphere { radius: 0.35 };

    let (rb, _col) =
        physics.add_dynamic_body(Vec3::new(0.0, 3.0, 0.0), shape.to_rapier_collider(), 0.45);

    assert!(physics.rigid_body_set.get(rb).unwrap().is_ccd_enabled());
}
