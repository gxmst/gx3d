use crate::core::{EngineWorld, Transform};
use crate::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld, Ray, RaycastHit};
use glam::Vec3;
use rapier3d::prelude::RigidBodyHandle;
use std::collections::VecDeque;

#[derive(Debug)]
pub struct PhysicsSandbox {
    pub held_body: Option<RigidBodyHandle>,
    pub hold_distance: f32,
    pub cube_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub sphere_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub box_material: crate::asset::Handle<crate::asset::Material>,
    pub bouncy_material: crate::asset::Handle<crate::asset::Material>,
    pub ice_material: crate::asset::Handle<crate::asset::Material>,
    pub heavy_material: crate::asset::Handle<crate::asset::Material>,
    pub runtime_props: VecDeque<hecs::Entity>,
    pub runtime_prop_limit: usize,
}

impl Default for PhysicsSandbox {
    fn default() -> Self {
        Self {
            held_body: None,
            hold_distance: 0.0,
            cube_mesh: Default::default(),
            sphere_mesh: Default::default(),
            box_material: Default::default(),
            bouncy_material: Default::default(),
            ice_material: Default::default(),
            heavy_material: Default::default(),
            runtime_props: VecDeque::new(),
            runtime_prop_limit: 96,
        }
    }
}

#[allow(clippy::too_many_arguments)]
impl PhysicsSandbox {
    pub fn spawn_prop(
        &mut self,
        world: &mut EngineWorld,
        physics: &mut PhysicsWorld,
        mesh: crate::asset::Handle<crate::asset::Mesh>,
        material: crate::asset::Handle<crate::asset::Material>,
        transform: Transform,
        shape: PhysicsShape,
        mass: f32,
    ) -> hecs::Entity {
        self.spawn_prop_with_physics_material(
            world,
            physics,
            mesh,
            material,
            transform,
            shape,
            mass,
            PhysicsMaterial::default(),
        )
    }

    pub fn spawn_prop_with_physics_material(
        &mut self,
        world: &mut EngineWorld,
        physics: &mut PhysicsWorld,
        mesh: crate::asset::Handle<crate::asset::Mesh>,
        material: crate::asset::Handle<crate::asset::Material>,
        transform: Transform,
        shape: PhysicsShape,
        mass: f32,
        physics_material: PhysicsMaterial,
    ) -> hecs::Entity {
        let collider = shape.to_rapier_collider_with_material(physics_material);
        let (rb, col) = if mass <= 0.0 {
            physics.add_static_body(transform.position, collider)
        } else {
            physics.add_dynamic_body(transform.position, collider, mass)
        };
        physics.set_body_rotation(rb, transform.rotation);

        let entity = world.spawn();
        world.add_component(entity, transform);
        world.add_component(entity, crate::renderer::RenderMesh { mesh, material });
        world.add_component(entity, PhysicsBody::new(rb, col, mass <= 0.0));
        physics.register_entity(col, entity);
        entity
    }

    pub fn spawn_dynamic_box(
        &mut self,
        world: &mut EngineWorld,
        physics: &mut PhysicsWorld,
        mesh: crate::asset::Handle<crate::asset::Mesh>,
        material: crate::asset::Handle<crate::asset::Material>,
        transform: Transform,
        half_size: f32,
        mass: f32,
    ) -> hecs::Entity {
        self.spawn_prop(
            world,
            physics,
            mesh,
            material,
            transform,
            PhysicsShape::Cuboid {
                half_extents: Vec3::splat(half_size),
            },
            mass,
        )
    }

    pub fn spawn_dynamic_sphere(
        &mut self,
        world: &mut EngineWorld,
        physics: &mut PhysicsWorld,
        mesh: crate::asset::Handle<crate::asset::Mesh>,
        material: crate::asset::Handle<crate::asset::Material>,
        transform: Transform,
        radius: f32,
        mass: f32,
        physics_material: PhysicsMaterial,
    ) -> hecs::Entity {
        self.spawn_prop_with_physics_material(
            world,
            physics,
            mesh,
            material,
            transform,
            PhysicsShape::Sphere { radius },
            mass,
            physics_material,
        )
    }

    pub fn ray_pick(
        &self,
        world: &EngineWorld,
        physics: &PhysicsWorld,
        ray: &Ray,
    ) -> Option<RigidBodyHandle> {
        self.body_from_hit(world, physics, physics.cast_ray(ray))
    }

    pub fn ray_pick_excluding(
        &self,
        world: &EngineWorld,
        physics: &PhysicsWorld,
        ray: &Ray,
        excluded: RigidBodyHandle,
    ) -> Option<RigidBodyHandle> {
        self.body_from_hit(
            world,
            physics,
            physics.cast_ray_excluding_body(ray, excluded),
        )
    }

    fn body_from_hit(
        &self,
        world: &EngineWorld,
        physics: &PhysicsWorld,
        hit: Option<RaycastHit>,
    ) -> Option<RigidBodyHandle> {
        hit.and_then(|hit: RaycastHit| {
            let entity = hit.entity?;
            world.ecs.get::<&PhysicsBody>(entity).ok().and_then(|body| {
                physics
                    .is_sandbox_manipulable(&body)
                    .then_some(body.rigid_body_handle)
            })
        })
    }

    pub fn hold_body(&mut self, body: RigidBodyHandle, hold_distance: f32) {
        self.held_body = Some(body);
        self.hold_distance = hold_distance.clamp(2.2, 10.0);
    }

    pub fn drop_body(&mut self) {
        self.held_body = None;
    }

    pub fn track_runtime_prop(
        &mut self,
        entity: hecs::Entity,
        world: &mut EngineWorld,
        physics: &mut PhysicsWorld,
    ) {
        self.runtime_props.push_back(entity);
        while self.runtime_props.len() > self.runtime_prop_limit.max(1) {
            let Some(oldest) = self.runtime_props.pop_front() else {
                break;
            };
            if let Some(body) = world.get_component::<PhysicsBody>(oldest) {
                if self.held_body == Some(body.rigid_body_handle) {
                    self.drop_body();
                }
                physics.remove_body(body.rigid_body_handle);
            }
            world.despawn(oldest);
        }
    }

    pub fn forget_runtime_prop(&mut self, entity: hecs::Entity) {
        self.runtime_props.retain(|tracked| *tracked != entity);
    }

    pub fn adjust_hold_distance(&mut self, scroll_delta: f32) {
        if self.held_body.is_some() {
            self.hold_distance = (self.hold_distance + scroll_delta * 0.35).clamp(2.2, 10.0);
        }
    }

    pub fn throw_held(
        &mut self,
        physics: &mut PhysicsWorld,
        camera_forward: Vec3,
        strength: f32,
    ) -> bool {
        let Some(body) = self.held_body.take() else {
            return false;
        };
        // A frozen prop can be held; make it dynamic again before launching.
        physics.unfreeze_body(body);
        physics.apply_impulse(body, camera_forward.normalize_or_zero() * strength);
        true
    }

    pub fn update_hold(
        &mut self,
        physics: &mut PhysicsWorld,
        camera_pos: Vec3,
        camera_forward: Vec3,
    ) {
        self.update_hold_excluding(physics, camera_pos, camera_forward, None);
    }

    pub fn update_hold_excluding(
        &mut self,
        physics: &mut PhysicsWorld,
        camera_pos: Vec3,
        camera_forward: Vec3,
        player_body: Option<RigidBodyHandle>,
    ) {
        let Some(body) = self.held_body else { return };
        let Some(current) = physics.get_body_position(body) else {
            self.drop_body();
            return;
        };
        if (current - camera_pos).length_squared() > 24.0 * 24.0 {
            self.drop_body();
            return;
        }

        let forward = camera_forward.normalize_or_zero();
        let desired_target = camera_pos + forward * self.hold_distance;
        let desired_translation = desired_target - current;
        let safe_translation =
            physics.sweep_body_translation(body, desired_translation, player_body);
        let target = current + safe_translation;
        let blocked =
            safe_translation.length_squared() + 1.0e-5 < desired_translation.length_squared();
        let velocity = physics.get_body_velocity(body).unwrap_or(Vec3::ZERO);
        let delta = target - current;
        // A bounded velocity servo is stable at any render framerate. Reduce
        // its top speed near obstacles to avoid tunnelling at sharp corners.
        let max_speed = if blocked { 6.0 } else { 11.0 };
        let desired = (delta * 7.0).clamp_length_max(max_speed);
        let dt = physics.integration_parameters.dt.max(0.0);
        let response = (1.0 - (-24.0 * dt).exp()).clamp(0.0, 1.0);
        let next_velocity = velocity.lerp(desired, response).clamp_length_max(max_speed);
        if let Some(rb) = physics.rigid_body_set.get_mut(body) {
            rb.wake_up(true);
            rb.set_linvel(next_velocity, true);
            rb.set_angvel(rb.angvel() * 0.72, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PhysicsSandbox;
    use crate::physics::{PhysicsShape, PhysicsWorld};
    use glam::Vec3;

    #[test]
    fn held_body_target_stays_in_front_of_wall() {
        let mut physics = PhysicsWorld::default();
        let (player, _) = physics.add_dynamic_body(
            Vec3::new(20.0, 0.0, 0.0),
            PhysicsShape::Sphere { radius: 0.3 }.to_rapier_collider(),
            1.0,
        );
        let (held, _) = physics.add_dynamic_body(
            Vec3::new(0.0, 0.0, 3.2),
            PhysicsShape::Sphere { radius: 0.35 }.to_rapier_collider(),
            1.0,
        );
        physics.add_static_body(
            Vec3::new(0.0, 0.0, 2.0),
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(2.0, 2.0, 0.1),
            }
            .to_rapier_collider(),
        );
        physics.step();

        let mut sandbox = PhysicsSandbox::default();
        sandbox.hold_body(held, 5.0);
        for _ in 0..30 {
            sandbox.update_hold_excluding(
                &mut physics,
                Vec3::new(0.0, 0.0, 5.0),
                -Vec3::Z,
                Some(player),
            );
            physics.step();
        }

        assert!(physics.get_body_position(held).unwrap().z > 2.15);
    }

    #[test]
    fn runtime_prop_limit_removes_oldest_body_and_entity() {
        let mut world = crate::core::EngineWorld::new();
        let mut physics = PhysicsWorld::default();
        let mut sandbox = PhysicsSandbox {
            runtime_prop_limit: 2,
            ..Default::default()
        };
        let mut spawned = Vec::new();
        for x in 0..3 {
            let entity = sandbox.spawn_dynamic_box(
                &mut world,
                &mut physics,
                Default::default(),
                Default::default(),
                crate::core::Transform::from_position(Vec3::new(x as f32, 2.0, 0.0)),
                0.5,
                1.0,
            );
            sandbox.track_runtime_prop(entity, &mut world, &mut physics);
            spawned.push(entity);
        }
        assert!(!world.ecs.contains(spawned[0]));
        assert!(world.ecs.contains(spawned[1]));
        assert!(world.ecs.contains(spawned[2]));
    }
}
