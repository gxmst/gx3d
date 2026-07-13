use glam::{Quat, Vec3};
use rapier3d::control::{CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::ShapeCastOptions;
use rapier3d::prelude::{
    BroadPhaseBvh, CCDSolver, Collider, ColliderHandle, ColliderSet, ImpulseJointSet,
    IntegrationParameters, IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline,
    QueryFilter, RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
};
use std::collections::HashMap;

const MAX_DYNAMIC_LINEAR_SPEED: f32 = 80.0;
const MAX_DYNAMIC_ANGULAR_SPEED: f32 = 80.0;

pub mod collider;
pub mod raycast;
pub mod sandbox;
pub mod world;

pub use collider::*;
pub use raycast::*;
pub use sandbox::*;
pub use world::*;

/// Result of one collision-constrained kinematic character movement.
#[derive(Debug, Clone, Copy)]
pub struct CharacterMoveResult {
    pub translation: Vec3,
    pub grounded: bool,
    pub hit_ceiling: bool,
    pub collision_count: usize,
}

fn vec3_to_rapier(v: Vec3) -> rapier3d::math::Vector {
    rapier3d::math::Vector::new(v.x, v.y, v.z)
}

fn quat_to_rapier(q: Quat) -> rapier3d::math::Rotation {
    q
}

fn quat_from_rapier(q: &rapier3d::math::Rotation) -> Quat {
    *q
}

fn character_length_value(length: CharacterLength, reference: f32) -> f32 {
    match length {
        CharacterLength::Absolute(value) => value,
        CharacterLength::Relative(fraction) => fraction * reference,
    }
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
        // A few extra solver/CCD passes are inexpensive for this small sandbox
        // and noticeably improve stacks and fast thrown props around the player.
        let integration_parameters = IntegrationParameters {
            num_solver_iterations: 8,
            max_ccd_substeps: 4,
            normalized_prediction_distance: 0.003,
            ..IntegrationParameters::default()
        };
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity,
            integration_parameters,
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

    /// Whether a body may be manipulated by the in-game physics sandbox.
    ///
    /// `PhysicsBody::is_static` records authored intent, while Rapier's body
    /// type records the current simulation mode. Checking both keeps frozen
    /// props selectable without accidentally treating kinematic actors (for
    /// example enemies and doors) as grabbable or deletable scenery.
    pub fn is_sandbox_manipulable(&self, body: &PhysicsBody) -> bool {
        !body.is_static
            && self
                .rigid_body_set
                .get(body.rigid_body_handle)
                .is_some_and(|rigid_body| rigid_body.is_dynamic() || rigid_body.is_fixed())
    }

    pub fn step(&mut self) {
        self.clamp_dynamic_velocities();
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
        self.clamp_dynamic_velocities();
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
        mut collider: Collider,
        mass: f32,
    ) -> (RigidBodyHandle, ColliderHandle) {
        let mass = if mass.is_finite() && mass > 0.0 {
            mass
        } else {
            1.0
        };
        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(vec3_to_rapier(position))
            .linear_damping(0.16)
            .angular_damping(0.48)
            .ccd_enabled(true)
            .soft_ccd_prediction(0.25)
            .build();
        // Scene `mass` is authored as total body mass, not an increment on top
        // of the collider's implicit density-derived mass.
        collider.set_mass(mass);
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
            .ccd_enabled(true)
            .soft_ccd_prediction(0.2)
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

    /// Teleport a body and clear all motion inherited from its old location.
    /// Position-based kinematic bodies need both their current and next poses
    /// updated, otherwise Rapier derives a huge one-frame velocity.
    pub fn teleport_body(&mut self, handle: RigidBodyHandle, position: Vec3) -> bool {
        if !position.is_finite() {
            return false;
        }
        let Some(body) = self.rigid_body_set.get_mut(handle) else {
            return false;
        };
        body.set_translation(vec3_to_rapier(position), true);
        if body.is_kinematic() {
            body.set_next_kinematic_translation(vec3_to_rapier(position));
        }
        body.set_linvel(Vec3::ZERO, true);
        body.set_angvel(Vec3::ZERO, true);
        self.refresh_body_colliders(&[handle]);
        true
    }

    /// Move a position-based kinematic body with Rapier's character controller.
    /// This gives the caller slope/step/ground snapping while shape casts keep
    /// even a large requested movement from tunnelling through thin geometry.
    /// Approximate impulses are transferred to dynamic bodies hit on the way.
    pub fn move_kinematic_character(
        &mut self,
        handle: RigidBodyHandle,
        desired_translation: Vec3,
        controller: &KinematicCharacterController,
        character_mass: f32,
    ) -> Option<CharacterMoveResult> {
        let dt = self.integration_parameters.dt;
        if !dt.is_finite() || dt <= 0.0 || !desired_translation.is_finite() {
            return None;
        }

        let (character_position, current_translation, collider_handle) = {
            let body = self.rigid_body_set.get(handle)?;
            if !body.is_kinematic() {
                return None;
            }
            (
                *body.position(),
                body.translation(),
                *body.colliders().first()?,
            )
        };
        let character_shape = self
            .collider_set
            .get(collider_handle)?
            .shared_shape()
            .clone();
        let filter = QueryFilter::default()
            .exclude_rigid_body(handle)
            .exclude_sensors();
        let mut collisions = Vec::new();
        let effective = {
            let queries = self.broad_phase.as_query_pipeline(
                self.narrow_phase.query_dispatcher(),
                &self.rigid_body_set,
                &self.collider_set,
                filter,
            );
            controller.move_shape(
                dt,
                &queries,
                character_shape.as_ref(),
                &character_position,
                desired_translation,
                |collision| collisions.push(collision),
            )
        };

        if !collisions.is_empty() && character_mass.is_finite() && character_mass > 0.0 {
            let mut queries = self.broad_phase.as_query_pipeline_mut(
                self.narrow_phase.query_dispatcher(),
                &mut self.rigid_body_set,
                &mut self.collider_set,
                filter,
            );
            controller.solve_character_collision_impulses(
                dt,
                &mut queries,
                character_shape.as_ref(),
                character_mass,
                collisions.iter(),
            );
        }

        let hit_ceiling = collisions
            .iter()
            .any(|collision| collision.hit.normal1.dot(controller.up) < -0.25);
        let mut final_translation = effective.translation;
        let mut grounded = effective.grounded;
        if !grounded && desired_translation.dot(controller.up) <= 1.0e-5 {
            let target_pose = rapier3d::math::Pose::from_parts(
                current_translation + final_translation,
                character_position.rotation,
            );
            let character_extent = character_shape
                .compute_local_aabb()
                .extents()
                .dot(controller.up.abs());
            let snap_distance = controller
                .snap_to_ground
                .map(|length| character_length_value(length, character_extent))
                .unwrap_or(0.0)
                .max(0.0);
            let target_distance =
                character_length_value(controller.offset, character_extent).max(0.0);
            let queries = self.broad_phase.as_query_pipeline(
                self.narrow_phase.query_dispatcher(),
                &self.rigid_body_set,
                &self.collider_set,
                filter,
            );
            if snap_distance > 0.0 {
                if let Some((_, hit)) = queries.cast_shape(
                    &target_pose,
                    -controller.up,
                    character_shape.as_ref(),
                    ShapeCastOptions {
                        max_time_of_impact: snap_distance,
                        target_distance,
                        stop_at_penetration: false,
                        compute_impact_geometry_on_penetration: true,
                    },
                ) {
                    if hit.normal1.dot(controller.up) > 0.5 {
                        final_translation -= controller.up * hit.time_of_impact;
                        grounded = true;
                    }
                }
            }
        }
        let target_translation = current_translation + final_translation;
        self.rigid_body_set
            .get_mut(handle)?
            .set_next_kinematic_translation(target_translation);

        Some(CharacterMoveResult {
            translation: final_translation,
            grounded,
            hit_ceiling,
            collision_count: collisions.len(),
        })
    }

    pub fn set_body_rotation(&mut self, handle: RigidBodyHandle, rotation: Quat) {
        if let Some(rb) = self.rigid_body_set.get_mut(handle) {
            rb.set_rotation(quat_to_rapier(rotation), true);
        }
    }

    /// Sweep a body's actual collider shape toward a target translation and
    /// return a collision-safe displacement. This is used by the physics grab
    /// tool so large props and corners cannot be pulled through thin walls.
    pub fn sweep_body_translation(
        &self,
        handle: RigidBodyHandle,
        desired_translation: Vec3,
        ignored_body: Option<RigidBodyHandle>,
    ) -> Vec3 {
        if !desired_translation.is_finite() || desired_translation.length_squared() < 1.0e-8 {
            return Vec3::ZERO;
        }
        let Some(body) = self.rigid_body_set.get(handle) else {
            return Vec3::ZERO;
        };
        let Some(collider_handle) = body.colliders().first().copied() else {
            return Vec3::ZERO;
        };
        let Some(collider) = self.collider_set.get(collider_handle) else {
            return Vec3::ZERO;
        };
        let ignore_predicate =
            |_: ColliderHandle, collider: &Collider| collider.parent() != ignored_body;
        let filter = QueryFilter::default()
            .exclude_rigid_body(handle)
            .exclude_sensors()
            .predicate(&ignore_predicate);
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );
        let options = ShapeCastOptions {
            max_time_of_impact: 1.0,
            target_distance: 0.035,
            stop_at_penetration: false,
            compute_impact_geometry_on_penetration: true,
        };
        let Some((_, hit)) = queries.cast_shape(
            body.position(),
            desired_translation,
            collider.shape(),
            options,
        ) else {
            return desired_translation;
        };
        let distance = desired_translation.length();
        let safe_time = (hit.time_of_impact - 0.025 / distance).clamp(0.0, 1.0);
        desired_translation * safe_time
    }

    /// Returns true when a proposed kinematic pose overlaps a movable body.
    /// Fixed level geometry is intentionally ignored so authored door frames
    /// can touch their doors without permanently locking them.
    pub fn movable_body_blocks_pose(
        &self,
        handle: RigidBodyHandle,
        position: Vec3,
        rotation: Quat,
    ) -> bool {
        if !position.is_finite() || !rotation.is_finite() {
            return true;
        }
        let Some(body) = self.rigid_body_set.get(handle) else {
            return false;
        };
        let Some(collider_handle) = body.colliders().first().copied() else {
            return false;
        };
        let Some(collider) = self.collider_set.get(collider_handle) else {
            return false;
        };
        let filter = QueryFilter::exclude_fixed()
            .exclude_rigid_body(handle)
            .exclude_sensors();
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.rigid_body_set,
            &self.collider_set,
            filter,
        );
        let pose = rapier3d::math::Pose::from_parts(position, rotation);
        let blocked = queries
            .intersect_shape(pose, collider.shape())
            .next()
            .is_some();
        blocked
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

    fn clamp_dynamic_velocities(&mut self) {
        for (_, body) in self.rigid_body_set.iter_mut() {
            if !body.is_dynamic() {
                continue;
            }
            let linear = clamped_finite_velocity(body.linvel(), MAX_DYNAMIC_LINEAR_SPEED);
            let angular = clamped_finite_velocity(body.angvel(), MAX_DYNAMIC_ANGULAR_SPEED);
            if linear != body.linvel() {
                body.set_linvel(linear, true);
            }
            if angular != body.angvel() {
                body.set_angvel(angular, true);
            }
        }
    }
}

fn clamped_finite_velocity(velocity: Vec3, maximum: f32) -> Vec3 {
    if !velocity.is_finite() {
        Vec3::ZERO
    } else {
        velocity.clamp_length_max(maximum)
    }
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new(Vec3::new(0.0, -9.81, 0.0))
    }
}
