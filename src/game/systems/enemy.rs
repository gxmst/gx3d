use crate::core::Transform;
use crate::core::{EngineWorld, Resources, Time};
use crate::game::EnemyAI;
use crate::physics::{PhysicsBody, PhysicsWorld};
use crate::renderer::RenderMesh;
use glam::{Quat, Vec3};

use super::{HitFlash, TimedEffect, WeaponFeedbackAssets};

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    // In match mode actors freeze during warmup / round-over phases.
    if !super::match_mode::movement_allowed(resources) {
        return;
    }
    let dt = resources
        .get::<Time>()
        .map(|t| t.fixed_timestep)
        .unwrap_or(0.0);

    let controller = crate::physics::character_controller(0.32, Some(0.32));
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
    // Match bots (BotBrain) revive next round: they get a death burst and are
    // sunk out of sight instead of being despawned.
    let dead: Vec<(
        hecs::Entity,
        Vec3,
        Option<rapier3d::prelude::RigidBodyHandle>,
        bool,
    )> = world
        .ecs
        .query::<(
            hecs::Entity,
            &EnemyAI,
            &Transform,
            Option<&PhysicsBody>,
            Option<&crate::game::BotBrain>,
        )>()
        .iter()
        .filter(|(_, ai, _, _, brain)| {
            // A bot "dies" once: skip ones already moved below the map.
            !(ai.is_alive || brain.is_some() && ai.health < 0.0)
        })
        .map(|(entity, _, transform, body, brain)| {
            (
                entity,
                transform.position,
                body.map(|b| b.rigid_body_handle),
                brain.is_some(),
            )
        })
        .collect();

    if dead.is_empty() {
        return;
    }

    let feedback = resources.get::<WeaponFeedbackAssets>().map(|f| *f);
    for (entity, position, body_handle, is_bot) in dead {
        if let Some(feedback) = feedback {
            spawn_death_burst(world, position, feedback);
        }
        if is_bot {
            // Park the bot far below the map until the next round revives it.
            // health < 0 marks "already processed" for the filter above.
            let parked = position - Vec3::Y * 500.0;
            if let Ok(mut transform) = world.ecs.get::<&mut Transform>(entity) {
                transform.position = parked;
            }
            if let Ok(mut ai) = world.ecs.get::<&mut EnemyAI>(entity) {
                ai.health = -1.0;
            }
            if let Some(handle) = body_handle {
                if let Some(mut physics) = resources.get_mut::<crate::physics::PhysicsWorld>() {
                    physics.teleport_body(handle, parked);
                }
            }
        } else {
            // Remove the physics body so the corpse leaves no ghost collider.
            if let Some(handle) = body_handle {
                if let Some(mut physics) = resources.get_mut::<crate::physics::PhysicsWorld>() {
                    physics.remove_body(handle);
                }
            }
            world.despawn(entity);
        }
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
