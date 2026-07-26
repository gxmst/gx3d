//! Visible weapons for AI actors (match bots and patrol enemies).
//!
//! Each armed actor gets a child "held rifle" entity that follows its hand
//! every frame, plus a brief muzzle-flash entity when it fires. Pure visuals:
//! combat logic stays in `combat.rs` / `match_mode.rs`, which report shots
//! through the [`BotShotFlashes`] queue.

use crate::core::{EngineWorld, Resources, Transform};
use crate::game::EnemyAI;
use crate::renderer::RenderMesh;
use glam::{Quat, Vec3};

/// Marks the held-weapon prop of one AI actor.
#[derive(Debug, Clone, Copy)]
pub struct BotWeapon {
    pub owner: hecs::Entity,
}

/// Muzzle flashes requested by combat systems this tick: world position and
/// facing of the shooter at fire time. Drained by `update`.
#[derive(Default)]
pub struct BotShotFlashes(pub Vec<(Vec3, Quat)>);

/// Local offset of the held rifle relative to the actor: right hand, chest
/// height, barrel forward. Actors face +Z locally (see EnemyAI::update).
const HOLD_OFFSET: Vec3 = Vec3::new(0.22, 0.42, 0.28);
const HOLD_SCALE: Vec3 = Vec3::new(0.55, 0.55, 0.55);

pub fn update(world: &mut EngineWorld, resources: &Resources) {
    ensure_weapons(world, resources);
    follow_owners(world);
    spawn_flashes(world, resources);
}

/// Give every armed actor (EnemyAI) exactly one weapon prop; despawn props
/// whose owner died or despawned.
fn ensure_weapons(world: &mut EngineWorld, resources: &Resources) {
    // Actors that should hold a weapon.
    let armed: Vec<hecs::Entity> = world
        .ecs
        .query::<(hecs::Entity, &EnemyAI)>()
        .iter()
        .filter(|(_, ai)| ai.is_alive)
        .map(|(entity, _)| entity)
        .collect();

    // Existing props by owner.
    let mut props: Vec<(hecs::Entity, hecs::Entity)> = world
        .ecs
        .query::<(hecs::Entity, &BotWeapon)>()
        .iter()
        .map(|(entity, weapon)| (entity, weapon.owner))
        .collect();

    // Remove orphaned props (owner gone or dead).
    let mut orphans = Vec::new();
    props.retain(|(prop, owner)| {
        let alive = world
            .ecs
            .get::<&EnemyAI>(*owner)
            .map(|ai| ai.is_alive)
            .unwrap_or(false);
        if !alive {
            orphans.push(*prop);
        }
        alive
    });
    for prop in orphans {
        world.despawn(prop);
    }

    // Spawn props for newly armed actors.
    let Some(meshes) = resources.get::<super::WeaponMeshes>() else {
        return;
    };
    // Bots visually carry the rifle-archetype mesh.
    let Some(rifle_mesh) = meshes
        .0
        .get(crate::game::weapon::SANDBOX_WEAPON_INDEX)
        .copied()
    else {
        return;
    };
    drop(meshes);
    let material = resources
        .get::<crate::physics::PhysicsSandbox>()
        .map(|sandbox| sandbox.heavy_material);
    let Some(material) = material else { return };

    for owner in armed {
        if props.iter().any(|(_, existing)| *existing == owner) {
            continue;
        }
        let prop = world.spawn();
        world.add_component(prop, Transform::new(Vec3::ZERO, Quat::IDENTITY, HOLD_SCALE));
        world.add_component(
            prop,
            RenderMesh {
                mesh: rifle_mesh,
                material,
            },
        );
        world.add_component(prop, BotWeapon { owner });
    }
}

/// Snap each prop to its owner's hand. Runs every frame after AI movement so
/// the rifle tracks facing exactly (no lag / interpolation needed at 11 cm).
fn follow_owners(world: &mut EngineWorld) {
    let poses: Vec<(hecs::Entity, Vec3, Quat)> = world
        .ecs
        .query::<(hecs::Entity, &BotWeapon)>()
        .iter()
        .filter_map(|(prop, weapon)| {
            world
                .ecs
                .get::<&Transform>(weapon.owner)
                .ok()
                .map(|owner| (prop, owner.position, owner.rotation))
        })
        .collect();
    for (prop, owner_pos, owner_rot) in poses {
        if let Ok(mut transform) = world.ecs.get::<&mut Transform>(prop) {
            transform.position = owner_pos + owner_rot * HOLD_OFFSET;
            // The first-person rifle mesh points its barrel toward -Z (view
            // space forward); actors face +Z, so flip the prop around Y.
            transform.rotation = owner_rot * Quat::from_rotation_y(std::f32::consts::PI);
        }
    }
}

/// Consume queued shots into short-lived muzzle flash entities at the barrel.
fn spawn_flashes(world: &mut EngineWorld, resources: &Resources) {
    let shots = {
        let Some(mut queue) = resources.get_mut::<BotShotFlashes>() else {
            return;
        };
        std::mem::take(&mut queue.0)
    };
    if shots.is_empty() {
        return;
    }
    let Some(assets) = resources.get::<super::WeaponFeedbackAssets>().map(|a| *a) else {
        return;
    };
    for (position, rotation) in shots {
        // Barrel tip: hand offset plus barrel length forward.
        let muzzle = position + rotation * (HOLD_OFFSET + Vec3::new(0.0, 0.05, 0.55));
        let flash = world.spawn();
        world.add_component(flash, Transform::new(muzzle, rotation, Vec3::splat(0.12)));
        world.add_component(
            flash,
            RenderMesh {
                mesh: assets.muzzle_flash_mesh,
                material: assets.muzzle_flash_material,
            },
        );
        world.add_component(flash, super::TimedEffect { remaining: 0.05 });
    }
}
