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
fn kinematic_actor_is_not_pickable_even_when_not_marked_static() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let sandbox = PhysicsSandbox::default();
    let (rb, col) = physics.add_kinematic_body(
        Vec3::ZERO,
        PhysicsShape::Capsule {
            radius: 0.4,
            half_height: 0.8,
        }
        .to_rapier_collider(),
    );
    let entity = world.spawn();
    world.add_component(entity, Transform::from_position(Vec3::ZERO));
    // Enemies historically used `is_static = false`; the actual Rapier body
    // type must still keep them out of the sandbox manipulation path.
    world.add_component(entity, PhysicsBody::new(rb, col, false));
    physics.register_entity(col, entity);
    physics.step();

    let ray = Ray::new(Vec3::new(0.0, 3.0, 0.0), -Vec3::Y, 10.0);
    assert_eq!(sandbox.ray_pick(&world, &physics, &ray), None);
    assert!(!physics.is_sandbox_manipulable(&PhysicsBody::new(rb, col, false)));
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

    let shape = PhysicsShape::Cuboid {
        half_extents: Vec3::splat(0.5),
    };
    let (rb, _col) =
        physics.add_dynamic_body(Vec3::new(0.0, 1.0, 0.0), shape.to_rapier_collider(), 1.0);

    assert!(!physics.body_is_frozen(rb));
    physics.freeze_body(rb);
    assert!(physics.body_is_frozen(rb));
    physics.unfreeze_body(rb);
    assert!(!physics.body_is_frozen(rb));
}

#[test]
fn dynamic_bodies_enable_ccd() {
    let mut physics = PhysicsWorld::default();
    let shape = PhysicsShape::Sphere { radius: 0.35 };

    let (rb, _col) =
        physics.add_dynamic_body(Vec3::new(0.0, 3.0, 0.0), shape.to_rapier_collider(), 0.45);

    assert!(physics.rigid_body_set.get(rb).unwrap().is_ccd_enabled());
}

#[test]
fn shot_impulse_at_contact_wakes_and_moves_a_ball() {
    let mut physics = PhysicsWorld::default();
    let (body, _collider) = physics.add_dynamic_body(
        Vec3::new(0.0, 1.0, 0.0),
        PhysicsShape::Sphere { radius: 0.5 }.to_rapier_collider(),
        0.55,
    );
    physics.apply_impulse_at_point(body, Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.25, 1.0, 0.0));

    let rigid_body = physics.rigid_body_set.get(body).unwrap();
    assert!(!rigid_body.is_sleeping());
    assert!(rigid_body.linvel().z < -1.0);
    assert!(rigid_body.angvel().length_squared() > 0.0);
}

#[test]
fn crosshair_raycast_ignores_player_body_and_hits_ball() {
    let mut physics = PhysicsWorld::default();
    let (player, _) = physics.add_dynamic_body(
        Vec3::ZERO,
        PhysicsShape::Capsule {
            radius: 0.3,
            half_height: 0.5,
        }
        .to_rapier_collider(),
        1.0,
    );
    let (_ball_body, ball_collider) = physics.add_dynamic_body(
        Vec3::new(0.0, 0.0, -3.0),
        PhysicsShape::Sphere { radius: 0.5 }.to_rapier_collider(),
        0.5,
    );
    physics.step();

    let ray = Ray::new(Vec3::ZERO, -Vec3::Z, 10.0);
    let hit = physics
        .cast_ray_excluding_body(&ray, player)
        .expect("the ball should be visible through the player's own collider");
    assert_eq!(hit.collider_handle, ball_collider);
}

#[test]
fn held_ball_velocity_is_bounded() {
    let mut physics = PhysicsWorld::default();
    let mut sandbox = PhysicsSandbox::default();
    let (body, _) = physics.add_dynamic_body(
        Vec3::new(0.0, 1.0, -2.0),
        PhysicsShape::Sphere { radius: 0.4 }.to_rapier_collider(),
        0.55,
    );
    sandbox.hold_body(body, 3.0);
    for _ in 0..8 {
        sandbox.update_hold(&mut physics, Vec3::new(0.0, 1.0, 0.0), -Vec3::Z);
    }
    assert!(physics.get_body_velocity(body).unwrap().length() <= 11.01);
}

#[test]
fn free_ball_angular_velocity_decays() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    let (body, _) = physics.add_dynamic_body(
        Vec3::ZERO,
        PhysicsShape::Sphere { radius: 0.5 }.to_rapier_collider(),
        1.0,
    );
    physics
        .rigid_body_set
        .get_mut(body)
        .unwrap()
        .set_angvel(Vec3::new(0.0, 12.0, 0.0), true);
    let before = physics.rigid_body_set.get(body).unwrap().angvel().length();
    for _ in 0..120 {
        physics.step();
    }
    let after = physics.rigid_body_set.get(body).unwrap().angvel().length();
    assert!(
        after < before * 0.5,
        "angular damping should visibly slow a rolling ball"
    );
}

#[test]
fn manually_rotated_body_is_immediately_visible_to_raycasts() {
    let mut physics = PhysicsWorld::default();
    let (body, _) = physics.add_static_body(
        Vec3::ZERO,
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(2.0, 0.2, 0.2),
        }
        .to_rapier_collider(),
    );
    physics.step();
    let ray = Ray::new(Vec3::new(0.0, 0.0, 3.0), -Vec3::Z, 10.0);
    let before = physics.cast_ray(&ray).unwrap().distance;

    physics.set_body_rotation(body, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    physics.refresh_body_colliders(&[body]);

    let after = physics.cast_ray(&ray).unwrap().distance;
    assert!(after < before - 1.0);
}
