use crate::core::Transform;
use crate::core::{EngineWorld, Resources, Time};
use crate::game::EnemyAI;
use crate::physics::{PhysicsBody, PhysicsWorld};
use crate::renderer::RenderMesh;
use glam::{Quat, Vec3};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};

use super::{HitFlash, TimedEffect, WeaponFeedbackAssets};

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.fixed_timestep)
        .unwrap_or(0.0);

    let controller = KinematicCharacterController {
        offset: CharacterLength::Absolute(0.025),
        slide: true,
        autostep: Some(CharacterAutostep {
            max_height: CharacterLength::Absolute(0.32),
            min_width: CharacterLength::Absolute(0.18),
            include_dynamic_bodies: false,
        }),
        max_slope_climb_angle: 46.0_f32.to_radians(),
        min_slope_slide_angle: 55.0_f32.to_radians(),
        snap_to_ground: Some(CharacterLength::Absolute(0.32)),
        normal_nudge_factor: 1.0e-3,
        ..Default::default()
    };
    let mut physics = resources.expect_mut::<PhysicsWorld>();
    for (transform, ai, body) in world
        .ecs
        .query::<(&mut Transform, &mut EnemyAI, Option<&PhysicsBody>)>()
        .iter()
    {
        let previous = transform.position;
        ai.update(transform, dt);
        let requested_horizontal = transform.position - previous;
        if let Some(body) = body {
            let requested = requested_horizontal + Vec3::NEG_Y * 0.08;
            if let Some(movement) = physics.move_kinematic_character(
                body.rigid_body_handle,
                requested,
                &controller,
                70.0,
            ) {
                transform.position = previous + movement.translation;
                let actual_horizontal =
                    movement.translation - controller.up * movement.translation.dot(controller.up);
                ai.report_constrained_movement(requested_horizontal, actual_horizontal, dt);
            } else {
                transform.position = previous;
                ai.report_constrained_movement(requested_horizontal, Vec3::ZERO, dt);
            }
        }
    }
}

/// Drives the visible side of getting shot: ticks the hit-flash timer and
/// restores each enemy's original material when it expires, then handles death
/// (spawn a burst, drop the physics body, despawn the entity).
pub fn feedback_system(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds().min(0.05))
        .unwrap_or(0.0);

    // --- Tick hit-flash timers; restore material when the flash ends. ---
    let mut flash_done: Vec<(hecs::Entity, crate::asset::Handle<crate::asset::Material>)> =
        Vec::new();
    for (entity, flash) in world.ecs.query::<(hecs::Entity, &mut HitFlash)>().iter() {
        flash.remaining -= dt;
        if flash.remaining <= 0.0 {
            flash_done.push((entity, flash.original_material));
        }
    }
    for (entity, original) in flash_done {
        if let Ok(mut mesh) = world.ecs.get::<&mut RenderMesh>(entity) {
            mesh.material = original;
        }
        let _ = world.ecs.remove_one::<HitFlash>(entity);
    }

    // --- Collect dead enemies and their last position + physics body. ---
    // (Decided in weapon.rs via `EnemyAI::take_damage`.) Gather first so the
    // query borrow is released before we mutate the world.
    let dead: Vec<(
        hecs::Entity,
        Vec3,
        Option<rapier3d::prelude::RigidBodyHandle>,
    )> = world
        .ecs
        .query::<(hecs::Entity, &EnemyAI, &Transform, Option<&PhysicsBody>)>()
        .iter()
        .filter(|(_, ai, _, _)| !ai.is_alive)
        .map(|(entity, _, transform, body)| {
            (
                entity,
                transform.position,
                body.map(|b| b.rigid_body_handle),
            )
        })
        .collect();

    if dead.is_empty() {
        return;
    }

    let feedback = resources.get::<WeaponFeedbackAssets>().map(|f| *f);
    for (entity, position, body_handle) in dead {
        if let Some(feedback) = feedback {
            spawn_death_burst(world, position, feedback);
        }
        // Remove the physics body so the corpse leaves no ghost collider.
        if let Some(handle) = body_handle {
            if let Some(mut physics) = resources.get_mut::<crate::physics::PhysicsWorld>() {
                physics.remove_body(handle);
            }
        }
        world.despawn(entity);
    }
}

/// Spawn a short-lived cluster of bright spheres where an enemy died, reusing
/// the impact-effect assets so the burst glows through bloom.
fn spawn_death_burst(world: &mut EngineWorld, position: Vec3, feedback: WeaponFeedbackAssets) {
    // A few offset spheres make the death read as a small pop rather than a
    // single dot. They auto-despawn via TimedEffect.
    let offsets = [
        Vec3::ZERO,
        Vec3::new(0.18, 0.12, 0.0),
        Vec3::new(-0.15, 0.05, 0.12),
        Vec3::new(0.05, 0.22, -0.1),
    ];
    for (i, offset) in offsets.iter().enumerate() {
        let scale = 0.22 - i as f32 * 0.03;
        let burst = world.spawn();
        world.add_component(
            burst,
            Transform::new(position + *offset, Quat::IDENTITY, Vec3::splat(scale)),
        );
        world.add_component(
            burst,
            RenderMesh {
                mesh: feedback.impact_mesh,
                material: feedback.impact_material,
            },
        );
        world.add_component(burst, TimedEffect { remaining: 0.18 });
    }
}
