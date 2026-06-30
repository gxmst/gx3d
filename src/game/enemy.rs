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
        let direction = target - transform.position;
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
}

pub struct EnemySpawner {
    pub spawn_points: Vec<Vec3>,
    pub waypoints: Vec<Vec3>,
    pub enemy_mesh: Option<crate::asset::Handle<crate::asset::Mesh>>,
    pub enemy_material: Option<crate::asset::Handle<crate::asset::Material>>,
}

impl EnemySpawner {
    pub fn new() -> Self {
        Self {
            spawn_points: Vec::new(),
            waypoints: Vec::new(),
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

        for spawn_point in &self.spawn_points {
            let entity = world.spawn();

            // Create transform with larger scale for visibility
            let mut transform = Transform::from_position(*spawn_point);
            transform.scale = Vec3::new(0.8, 0.8, 0.8);
            world.add_component(entity, transform);

            let ai = EnemyAI::new(self.waypoints.clone(), 2.0, 100.0);
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
