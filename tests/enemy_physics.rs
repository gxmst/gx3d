use glam::Vec3;
use gxengine::core::{EngineWorld, Resources, Time, Transform};
use gxengine::game::{EnemyAI, EnemySpawner};
use gxengine::physics::{PhysicsBody, PhysicsShape, PhysicsWorld};

#[test]
fn spawned_enemies_have_physics_identity() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let spawner = EnemySpawner {
        spawn_points: vec![Vec3::new(1.0, 2.0, 3.0)],
        waypoints: vec![Vec3::new(2.0, 2.0, 3.0)],
        patrol_routes: Vec::new(),
        enemy_mesh: None,
        enemy_material: None,
    };

    let enemies = spawner.spawn_enemies(&mut world, &mut physics);

    assert_eq!(enemies.len(), 1);
    let enemy = enemies[0];
    assert!(world.has_component::<Transform>(enemy));
    assert!(world.has_component::<EnemyAI>(enemy));
    assert!(world.has_component::<PhysicsBody>(enemy));

    let body = world.get_component::<PhysicsBody>(enemy).unwrap();
    assert_eq!(physics.get_entity(body.collider_handle), Some(enemy));
}

#[test]
fn patrolling_enemy_stays_grounded_and_cannot_walk_through_a_wall() {
    let mut world = EngineWorld::new();
    let resources = Resources::new();
    let mut physics = PhysicsWorld::new(Vec3::ZERO);
    physics.add_static_body(
        Vec3::new(0.0, -0.5, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(10.0, 0.5, 10.0),
        }
        .to_rapier_collider(),
    );
    physics.add_static_body(
        Vec3::new(0.8, 1.0, 0.0),
        PhysicsShape::Cuboid {
            half_extents: Vec3::new(0.05, 1.0, 2.0),
        }
        .to_rapier_collider(),
    );
    let spawner = EnemySpawner {
        spawn_points: vec![Vec3::new(0.0, 1.2, 0.0)],
        waypoints: vec![Vec3::new(2.0, 1.2, 0.0)],
        patrol_routes: Vec::new(),
        enemy_mesh: None,
        enemy_material: None,
    };
    let enemy = spawner.spawn_enemies(&mut world, &mut physics)[0];
    physics.step();
    resources.insert(Time::new());
    resources.insert(physics);

    for _ in 0..120 {
        gxengine::game::systems::enemy::system(&mut world, &resources);
        resources.expect_mut::<PhysicsWorld>().step();
    }

    let transform = world.get_component::<Transform>(enemy).unwrap();
    assert!(transform.position.x < 0.38, "{transform:?}");
    assert!((transform.position.y - 1.2).abs() < 0.05, "{transform:?}");
}
