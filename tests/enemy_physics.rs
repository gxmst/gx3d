use glam::Vec3;
use gxengine::core::{EngineWorld, Transform};
use gxengine::game::{EnemyAI, EnemySpawner};
use gxengine::physics::{PhysicsBody, PhysicsWorld};

#[test]
fn spawned_enemies_have_physics_identity() {
    let mut world = EngineWorld::new();
    let mut physics = PhysicsWorld::default();
    let spawner = EnemySpawner {
        spawn_points: vec![Vec3::new(1.0, 2.0, 3.0)],
        waypoints: vec![Vec3::new(2.0, 2.0, 3.0)],
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
