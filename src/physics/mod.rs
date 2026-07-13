use glam::{Quat, Vec3};
use rapier3d::prelude::{
    BroadPhaseBvh, CCDSolver, Collider, ColliderHandle, ColliderSet, ImpulseJointSet,
    IntegrationParameters, IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline,
    RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
};
use std::collections::HashMap;

pub mod collider;
pub mod raycast;
pub mod sandbox;
pub mod world;

pub use collider::*;
pub use raycast::*;
pub use sandbox::*;
pub use world::*;

fn vec3_to_rapier(v: Vec3) -> rapier3d::math::Vector {
    rapier3d::math::Vector::new(v.x, v.y, v.z)
}

fn quat_to_rapier(q: Quat) -> rapier3d::math::Rotation {
    q
}

fn quat_from_rapier(q: &rapier3d::math::Rotation) -> Quat {
    *q
}

pub struct PhysicsWorld {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vec3,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhaseBvh,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub collider_entity_map: HashMap<ColliderHandle, hecs::Entity>,
}

impl PhysicsWorld {
    pub fn new(gravity: Vec3) -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity,
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            collider_entity_map: HashMap::new(),
        }
    }

    pub fn register_entity(&mut self, collider_handle: ColliderHandle, entity: hecs::Entity) {
        self.collider_entity_map.insert(collider_handle, entity);
    }

    pub fn get_entity(&self, collider_handle: ColliderHandle) -> Option<hecs::Entity> {
        self.collider_entity_map.get(&collider_handle).copied()
    }

    pub fn step(&mut self) {
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &(),
        );
    }

    pub fn add_static_body(
        &mut self,
        position: Vec3,
        collider: Collider,
    ) -> (RigidBodyHandle, ColliderHandle) {
        let rigid_body = RigidBodyBuilder::fixed()
            .translation(vec3_to_rapier(position))
            .build();
        let rb_handle = self.rigid_body_set.insert(rigid_body);
        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);
        (rb_handle, collider_handle)
    }

    pub fn add_dynamic_body(
        &mut self,
        position: Vec3,
        collider: Collider,
        mass: f32,
    ) -> (RigidBodyHandle, ColliderHandle) {
        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(vec3_to_rapier(position))
            .additional_mass(mass)
            .linear_damping(0.16)
            .angular_damping(0.48)
            .ccd_enabled(true)
            .soft_ccd_prediction(0.25)
            .build();
        let rb_handle = self.rigid_body_set.insert(rigid_body);
        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);
        (rb_handle, collider_handle)
    }

    pub fn add_kinematic_body(
        &mut self,
        position: Vec3,
        collider: Collider,
    ) -> (RigidBodyHandle, ColliderHandle) {
        let rigid_body = RigidBodyBuilder::kinematic_position_based()
            .translation(vec3_to_rapier(position))
            .build();
        let rb_handle = self.rigid_body_set.insert(rigid_body);
        let collider_handle =
            self.collider_set
                .insert_with_parent(collider, rb_handle, &mut self.rigid_body_set);
        (rb_handle, collider_handle)
    }

    pub fn get_body_position(&self, handle: RigidBodyHandle) -> Option<Vec3> {
        self.rigid_body_set.get(handle).map(|rb| {
            let pos = rb.translation();
            Vec3::new(pos.x, pos.y, pos.z)
        })
    }

    pub fn get_body_rotation(&self, handle: RigidBodyHandle) -> Option<Quat> {
        self.rigid_body_set
            .get(handle)
            .map(|rb| quat_from_rapier(rb.rotation()))
    }

    pub fn get_body_velocity(&self, handle: RigidBodyHandle) -> Option<Vec3> {
        self.rigid_body_set.get(handle).map(|rb| {
            let vel = rb.linvel();
            Vec3::new(vel.x, vel.y, vel.z)
        })
    }

    pub fn set_body_position(&mut self, handle: RigidBodyHandle, position: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.set_translation(vec3_to_rapier(position), true);
        }
    }

    pub fn set_body_rotation(&mut self, handle: RigidBodyHandle, rotation: Quat) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.set_rotation(quat_to_rapier(rotation), true);
        }
    }

    /// Make manually moved bodies immediately visible to raycasts, even when
    /// the simulation is paused and `step` will not run this frame.
    pub fn refresh_body_colliders(&mut self, bodies: &[RigidBodyHandle]) {
        let collider_handles: Vec<_> = bodies
            .iter()
            .filter_map(|handle| self.rigid_body_set.get(*handle))
            .flat_map(|body| body.colliders().iter().copied())
            .collect();
        self.rigid_body_set
            .propagate_modified_body_positions_to_colliders(&mut self.collider_set);
        for handle in collider_handles {
            if let Some(collider) = self.collider_set.get(handle) {
                let aabb = collider
                    .compute_broad_phase_aabb(&self.integration_parameters, &self.rigid_body_set);
                self.broad_phase
                    .set_aabb(&self.integration_parameters, handle, aabb);
            }
        }
    }

    pub fn set_body_velocity(&mut self, handle: RigidBodyHandle, velocity: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.set_linvel(vec3_to_rapier(velocity), true);
        }
    }

    pub fn set_body_horizontal_velocity(&mut self, handle: RigidBodyHandle, horizontal: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            let current_vel = rb.linvel();
            rb.set_linvel(
                rapier3d::math::Vector::new(horizontal.x, current_vel.y, horizontal.z),
                true,
            );
        }
    }

    pub fn apply_force(&mut self, handle: RigidBodyHandle, force: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.add_force(vec3_to_rapier(force), true);
        }
    }

    pub fn apply_impulse(&mut self, handle: RigidBodyHandle, impulse: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.apply_impulse(vec3_to_rapier(impulse), true);
        }
    }

    pub fn apply_impulse_at_point(&mut self, handle: RigidBodyHandle, impulse: Vec3, point: Vec3) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.wake_up(true);
            rb.apply_impulse_at_point(vec3_to_rapier(impulse), vec3_to_rapier(point), true);
        }
    }

    pub fn collider_body(&self, collider: ColliderHandle) -> Option<RigidBodyHandle> {
        self.collider_set
            .get(collider)
            .and_then(|collider| collider.parent())
    }

    /// Fully remove a rigid body, its attached colliders, and any
    /// collider→entity mappings. Without this, despawning an entity would
    /// leave a "ghost" collider behind that raycasts still hit.
    pub fn remove_body(&mut self, handle: RigidBodyHandle) {
        // Drop entity mappings for every collider attached to this body before
        // the body (and its colliders) are removed from the sets.
        if let Some(rb) = self.rigid_body_set.get(handle) {
            for collider_handle in rb.colliders() {
                self.collider_entity_map.remove(collider_handle);
            }
        }
        self.rigid_body_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
    }
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new(Vec3::new(0.0, -9.81, 0.0))
    }
}
