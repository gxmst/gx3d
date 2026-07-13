use glam::Vec3;
use gxengine::physics::{PhysicsShape, PhysicsWorld};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};

fn character_controller(with_autostep: bool) -> KinematicCharacterController {
    KinematicCharacterController {
        offset: CharacterLength::Absolute(0.02),
        slide: true,
        autostep: with_autostep.then_some(CharacterAutostep {
            max_height: CharacterLength::Absolute(0.36),
            min_width: CharacterLength::Absolute(0.18),
            include_dynamic_bodies: false,
        }),
        max_slope_climb_angle: 48.0_f32.to_radians(),
        min_slope_slide_angle: 54.0_f32.to_radians(),
        snap_to_ground: Some(CharacterLength::Absolute(0.24)),
        normal_nudge_factor: 1.0e-3,
        ..Default::default()
    }
}

fn add_player(physics: &mut PhysicsWorld, position: Vec3) -> rapier3d::prelude::RigidBodyHandle {
    let (body, _) = physics.add_kinematic_body(
        position,
        PhysicsShape::Capsule {
            radius: 0.3,
            half_height: 0.5,
        }
        .to_rapier_collider(),
    );
    body
}

fn add_ground(physics: &mut PhysicsWorld) {
    physics.add_static_body(
        Vec3::new(0.0, -0.5, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(20.0, 0.5, 20.0),
        }
        .to_rapier_collider(),
    );
}

#[test]
fn grounded_comes_from_floor_contact_not_vertical_velocity() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    add_ground(&mut physics);
    let player = add_player(&mut physics, Vec3::new(0.0, 0.8, 0.0));
    physics.step();

    let movement = physics
        .move_kinematic_character(
            player,
            Vec3::new(0.08, -0.025, 0.0),
            &character_controller(true),
            75.0,
        )
        .unwrap();

    assert!(movement.grounded, "{movement:?}");
    assert!(movement.translation.y > -0.03);
}

#[test]
fn character_shape_cast_cannot_tunnel_through_a_thin_wall() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    add_ground(&mut physics);
    let player = add_player(&mut physics, Vec3::new(0.0, 0.8, 0.0));
    physics.add_static_body(
        Vec3::new(2.0, 1.0, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(0.025, 1.0, 2.0),
        }
        .to_rapier_collider(),
    );
    physics.step();

    let movement = physics
        .move_kinematic_character(
            player,
            Vec3::new(10.0, -0.025, 0.0),
            &character_controller(true),
            75.0,
        )
        .unwrap();

    assert!(movement.collision_count > 0);
    assert!(movement.translation.x < 1.75);
}

#[test]
fn character_controller_steps_over_a_low_obstacle() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    add_ground(&mut physics);
    let player = add_player(&mut physics, Vec3::new(0.0, 0.8, 0.0));
    physics.add_static_body(
        Vec3::new(0.8, 0.15, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(0.25, 0.15, 1.0),
        }
        .to_rapier_collider(),
    );
    physics.step();

    let movement = physics
        .move_kinematic_character(
            player,
            Vec3::new(1.1, -0.025, 0.0),
            &character_controller(true),
            75.0,
        )
        .unwrap();

    assert!(movement.translation.x > 0.75, "{movement:?}");
    assert!(movement.translation.y > 0.15, "{movement:?}");
    assert!(movement.grounded, "{movement:?}");
}

#[test]
fn character_collision_pushes_a_dynamic_prop_without_overlapping_it() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    let player = add_player(&mut physics, Vec3::new(0.0, 0.8, 0.0));
    let (prop, _) = physics.add_dynamic_body(
        Vec3::new(1.0, 0.8, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::splat(0.3),
        }
        .to_rapier_collider(),
        2.0,
    );
    physics.step();

    let movement = physics
        .move_kinematic_character(
            player,
            Vec3::new(1.0, 0.0, 0.0),
            &character_controller(false),
            75.0,
        )
        .unwrap();

    assert!(movement.translation.x < 0.5);
    assert!(physics.get_body_velocity(prop).unwrap().x > 0.0);
}

#[test]
fn runaway_dynamic_velocities_are_sanitized_and_capped() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    let (body, _) = physics.add_dynamic_body(
        Vec3::ZERO,
        PhysicsShape::Sphere { radius: 0.3 }.to_rapier_collider(),
        1.0,
    );
    physics.set_body_velocity(body, Vec3::new(1_000.0, 0.0, 0.0));

    physics.step();

    assert!(physics.get_body_velocity(body).unwrap().length() <= 80.01);
}

#[test]
fn teleporting_a_kinematic_character_does_not_create_a_launch_velocity() {
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    let player = add_player(&mut physics, Vec3::ZERO);
    physics.step();

    assert!(physics.teleport_body(player, Vec3::new(4.0, 3.0, -2.0)));
    physics.step();

    assert_eq!(
        physics.get_body_position(player).unwrap(),
        Vec3::new(4.0, 3.0, -2.0)
    );
    assert!(physics.get_body_velocity(player).unwrap().length() < 1.0e-4);
}
