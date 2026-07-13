use crate::core::{EngineWorld, Transform};
use crate::physics::{PhysicsBody, PhysicsMaterial, PhysicsShape, PhysicsWorld, Ray, RaycastHit};
use glam::Vec3;
use rapier3d::prelude::{RigidBodyHandle, RigidBodyType};

#[derive(Debug, Default)]
pub struct PhysicsSandbox {
    pub held_body: Option<RigidBodyHandle>,
    pub hold_distance: f32,
    pub cube_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub sphere_mesh: crate::asset::Handle<crate::asset::Mesh>,
    pub box_material: crate::asset::Handle<crate::asset::Material>,
    pub bouncy_material: crate::asset::Handle<crate::asset::Material>,
    pub ice_material: crate::asset::Handle<crate::asset::Material>,
    pub heavy_material: crate::asset::Handle<crate::asset::Material>,
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
        self.body_from_hit(world, physics.cast_ray(ray))
    }

    pub fn ray_pick_excluding(
        &self,
        world: &EngineWorld,
        physics: &PhysicsWorld,
        ray: &Ray,
        excluded: RigidBodyHandle,
    ) -> Option<RigidBodyHandle> {
        self.body_from_hit(world, physics.cast_ray_excluding_body(ray, excluded))
    }

    fn body_from_hit(
        &self,
        world: &EngineWorld,
        hit: Option<RaycastHit>,
    ) -> Option<RigidBodyHandle> {
        hit.and_then(|hit: RaycastHit| {
            let entity = hit.entity?;
            world
                .ecs
                .get::<&PhysicsBody>(entity)
                .ok()
                .and_then(|b| (!b.is_static).then_some(b.rigid_body_handle))
        })
    }

    pub fn hold_body(&mut self, body: RigidBodyHandle, hold_distance: f32) {
        self.held_body = Some(body);
        self.hold_distance = hold_distance.clamp(2.2, 10.0);
    }

    pub fn drop_body(&mut self) {
        self.held_body = None;
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
        if let Some(rb) = physics.rigid_body_set.get_mut(body) {
            if rb.is_fixed() {
                rb.set_body_type(RigidBodyType::Dynamic, true);
            }
        }
        physics.apply_impulse(body, camera_forward.normalize_or_zero() * strength);
        true
    }

    pub fn update_hold(&self, physics: &mut PhysicsWorld, camera_pos: Vec3, camera_forward: Vec3) {
        if let Some(body) = self.held_body {
            let target = camera_pos + camera_forward * self.hold_distance;
            if let Some(current) = physics.get_body_position(body) {
                let velocity = physics.get_body_velocity(body).unwrap_or(Vec3::ZERO);
                let delta = target - current;
                // A bounded velocity servo is stable at any render framerate.
                // The previous force spring accumulated multiple times before
                // each 60 Hz physics tick and launched lightweight balls.
                let desired = (delta * 7.0).clamp_length_max(11.0);
                let next_velocity = velocity.lerp(desired, 0.32);
                if let Some(rb) = physics.rigid_body_set.get_mut(body) {
                    rb.wake_up(true);
                    rb.set_linvel(next_velocity, true);
                    rb.set_angvel(rb.angvel() * 0.72, true);
                }
            }
        }
    }

    pub fn freeze(&self, physics: &mut PhysicsWorld, body: RigidBodyHandle) {
        if let Some(rb) = physics.rigid_body_set.get_mut(body) {
            rb.set_body_type(RigidBodyType::Fixed, true);
            rb.set_linvel(Vec3::ZERO, true);
            rb.set_angvel(Vec3::ZERO, true);
        }
    }

    pub fn unfreeze(&self, physics: &mut PhysicsWorld, body: RigidBodyHandle) {
        if let Some(rb) = physics.rigid_body_set.get_mut(body) {
            rb.set_body_type(RigidBodyType::Dynamic, true);
        }
    }

    pub fn is_frozen(&self, physics: &PhysicsWorld, body: RigidBodyHandle) -> bool {
        physics
            .rigid_body_set
            .get(body)
            .map(|rb| rb.is_fixed())
            .unwrap_or(false)
    }
}
