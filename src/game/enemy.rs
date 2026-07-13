use crate::core::Transform;
use glam::Vec3;
use hecs::Entity;

#[derive(Debug, Clone)]
pub struct EnemyAI {
    pub waypoints: Vec<Vec3>,
    pub current_waypoint: usize,
    pub speed: f32,
    pub health: f32,
    pub max_health: f32,
    pub is_alive: bool,
    pub waypoint_threshold: f32,
    /// Time spent unable to make meaningful progress toward the current
    /// waypoint. After a short timeout the patrol advances instead of pushing
    /// forever into a wall or blocked doorway.
    pub blocked_timer: f32,
    /// Seconds of "stagger" remaining: while > 0 the enemy holds still so a hit
    /// reads as a visible flinch instead of uninterrupted patrolling.
    pub stagger_timer: f32,
}

impl EnemyAI {
    pub fn new(waypoints: Vec<Vec3>, speed: f32, health: f32) -> Self {
        Self {
            waypoints,
            current_waypoint: 0,
            speed,
            health,
            max_health: health,
            is_alive: true,
            waypoint_threshold: 0.5,
            blocked_timer: 0.0,
            stagger_timer: 0.0,
        }
    }

    pub fn update(&mut self, transform: &mut Transform, dt: f32) {
        if !self.is_alive || self.waypoints.is_empty() {
            return;
        }

        // While staggered from a recent hit, stand still and tick the timer down.
        if self.stagger_timer > 0.0 {
            self.stagger_timer -= dt;
            return;
        }

        let target = self.waypoints[self.current_waypoint];
        let mut direction = target - transform.position;
        // Patrol routes are authored on a 3D map, but vertical placement is
        // resolved by the physics controller. Ignoring waypoint altitude here
        // prevents actors from hovering toward a stale Y value on ramps and
        // raised bomb sites.
        direction.y = 0.0;
        let distance = direction.length();

        if distance < self.waypoint_threshold {
            // Reached waypoint, move to next
            self.current_waypoint = (self.current_waypoint + 1) % self.waypoints.len();
        } else {
            // Move towards waypoint
            let movement = direction.normalize() * self.speed * dt;
            transform.position += movement;

            // Face movement direction
            if direction.length_squared() > 0.001 {
                let forward = direction.normalize();
                transform.rotation = glam::Quat::from_rotation_arc(Vec3::Z, forward);
            }
        }
    }

    pub fn take_damage(&mut self, damage: f32) {
        self.health -= damage;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.is_alive = false;
        }
    }

    pub fn report_constrained_movement(&mut self, requested: Vec3, actual: Vec3, dt: f32) {
        let requested_distance = requested.length();
        let progress = if requested_distance > 1.0e-4 {
            actual.length() / requested_distance
        } else {
            1.0
        };
        if requested_distance > 1.0e-3 && progress < 0.15 {
            self.blocked_timer += dt.max(0.0);
            if self.blocked_timer >= 0.75 && !self.waypoints.is_empty() {
                self.current_waypoint = (self.current_waypoint + 1) % self.waypoints.len();
                self.blocked_timer = 0.0;
            }
        } else {
            self.blocked_timer = 0.0;
        }
    }
}

pub struct EnemySpawner {
    pub spawn_points: Vec<Vec3>,
    pub waypoints: Vec<Vec3>,
    pub patrol_routes: Vec<Vec<Vec3>>,
    pub enemy_mesh: Option<crate::asset::Handle<crate::asset::Mesh>>,
    pub enemy_material: Option<crate::asset::Handle<crate::asset::Material>>,
}

impl EnemySpawner {
    pub fn new() -> Self {
        Self {
            spawn_points: Vec::new(),
            waypoints: Vec::new(),
            patrol_routes: Vec::new(),
            enemy_mesh: None,
            enemy_material: None,
        }
    }

    pub fn spawn_enemies(
        &self,
        world: &mut crate::core::EngineWorld,
        physics_world: &mut crate::physics::PhysicsWorld,
    ) -> Vec<Entity> {
        let mut enemies = Vec::new();

        for (index, spawn_point) in self.spawn_points.iter().enumerate() {
            let entity = world.spawn();

            // Create transform with larger scale for visibility
            let mut transform = Transform::from_position(*spawn_point);
            transform.scale = Vec3::new(0.8, 0.8, 0.8);
            world.add_component(entity, transform);

            let waypoints = self
                .patrol_routes
                .get(index)
                .filter(|route| !route.is_empty())
                .unwrap_or(&self.waypoints)
                .clone();
            let ai = EnemyAI::new(waypoints, 2.0, 100.0);
            world.add_component(entity, ai);

            if let (Some(mesh), Some(material)) = (self.enemy_mesh, self.enemy_material) {
                world.add_component(entity, crate::renderer::RenderMesh { mesh, material });
            }

            // Add physics body for enemy
            let collider = crate::physics::PhysicsShape::Capsule {
                radius: 0.4,
                half_height: 0.8,
            };
            let (rb_handle, col_handle) =
                physics_world.add_kinematic_body(*spawn_point, collider.to_rapier_collider());
            world.add_component(
                entity,
                crate::physics::PhysicsBody::new(rb_handle, col_handle, false),
            );
            physics_world.register_entity(col_handle, entity);

            enemies.push(entity);
        }

        enemies
    }
}

impl Default for EnemySpawner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{EnemyAI, EnemySpawner};
    use crate::core::EngineWorld;
    use crate::physics::PhysicsWorld;
    use glam::Vec3;

    #[test]
    fn spawner_assigns_each_enemy_its_own_route() {
        let mut world = EngineWorld::new();
        let mut physics = PhysicsWorld::default();
        let mut spawner = EnemySpawner::new();
        spawner.spawn_points = vec![Vec3::ZERO, Vec3::X * 3.0];
        spawner.waypoints = vec![Vec3::Z];
        spawner.patrol_routes = vec![vec![Vec3::X], vec![-Vec3::X]];

        let enemies = spawner.spawn_enemies(&mut world, &mut physics);

        assert_eq!(
            world
                .get_component::<EnemyAI>(enemies[0])
                .unwrap()
                .waypoints,
            vec![Vec3::X]
        );
        assert_eq!(
            world
                .get_component::<EnemyAI>(enemies[1])
                .unwrap()
                .waypoints,
            vec![-Vec3::X]
        );
    }

    #[test]
    fn blocked_patrol_skips_a_stuck_waypoint() {
        let mut ai = EnemyAI::new(vec![Vec3::X, Vec3::Z], 2.0, 100.0);
        for _ in 0..46 {
            ai.report_constrained_movement(Vec3::X * 0.1, Vec3::ZERO, 1.0 / 60.0);
        }
        assert_eq!(ai.current_waypoint, 1);
    }

    #[test]
    fn patrol_motion_does_not_chase_waypoint_altitude() {
        let mut ai = EnemyAI::new(vec![Vec3::new(1.0, 100.0, 0.0)], 2.0, 100.0);
        let mut transform = crate::core::Transform::from_position(Vec3::new(0.0, 2.0, 0.0));
        ai.update(&mut transform, 0.25);
        assert_eq!(transform.position.y, 2.0);
        assert!(transform.position.x > 0.0);
    }
}
