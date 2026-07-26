//! Enemy fire, player health, death and respawn.
//!
//! Runs at the fixed tick right after the enemy patrol system. Each enemy with
//! line of sight to the player warms up briefly, then fires hitscan shots with
//! distance-scaled accuracy. Hits reduce [`PlayerHealth`]; at zero the player
//! respawns at the scene spawn with a short grace period.

use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::EnemyAI;
use crate::physics::{PhysicsWorld, Ray};
use crate::renderer::Camera;
use glam::Vec3;

use super::{PlayerBody, PlayerHealth, Toast};

/// Enemies only engage inside this range (meters).
const ENGAGE_RANGE: f32 = 30.0;
/// Continuous line-of-sight seconds required before the first shot.
const AIM_WARMUP_SECONDS: f32 = 0.6;
/// Seconds between shots from one enemy.
const FIRE_INTERVAL: f32 = 1.4;
/// Base damage per hit at point-blank range.
const HIT_DAMAGE: f32 = 14.0;
/// Post-respawn invulnerability seconds.
const RESPAWN_GRACE: f32 = 2.0;
/// Enemy muzzle height offset from the capsule origin.
const MUZZLE_HEIGHT: f32 = 0.6;

pub fn fixed_update(world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        return;
    }
    // Match mode has its own team-aware combat loop.
    if resources.contains::<super::match_mode::MatchState>() {
        return;
    }
    // God mode: observers are not shot at.
    if resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode == super::ViewMode::God)
        .unwrap_or(false)
    {
        return;
    }
    let dt = resources
        .get::<Time>()
        .map(|t| t.fixed_timestep)
        .unwrap_or(0.0);
    if dt <= 0.0 {
        return;
    }

    {
        let mut health = resources.expect_mut::<PlayerHealth>();
        health.invulnerable = (health.invulnerable - dt).max(0.0);
    }

    let player_position = {
        let camera = resources.expect::<Camera>();
        camera.position
    };
    let player_body = resources.expect::<PlayerBody>().0;

    // Collect firing decisions first (immutable physics queries), then apply.
    let mut total_damage = 0.0_f32;
    let mut shots: Vec<(Vec3, bool)> = Vec::new();
    {
        let invulnerable = resources.expect::<PlayerHealth>().invulnerable > 0.0;
        let physics = resources.expect::<PhysicsWorld>();
        for (transform, ai) in world.ecs.query::<(&mut Transform, &mut EnemyAI)>().iter() {
            if !ai.is_alive {
                continue;
            }
            ai.fire_cooldown = (ai.fire_cooldown - dt).max(0.0);

            let muzzle = transform.position + Vec3::Y * MUZZLE_HEIGHT;
            let to_player = player_position - muzzle;
            let distance = to_player.length();
            if !(0.5..=ENGAGE_RANGE).contains(&distance) {
                ai.aim_warmup = 0.0;
                continue;
            }
            // Line of sight: the first thing the ray hits must be the player.
            let ray = Ray::new(muzzle, to_player, distance + 0.5);
            let sees_player = physics
                .cast_ray(&ray)
                .and_then(|hit| physics.collider_body(hit.collider_handle))
                .map(|body| body == player_body)
                .unwrap_or(false);
            if !sees_player {
                ai.aim_warmup = 0.0;
                continue;
            }

            // Face the player while engaging (overrides patrol facing).
            let flat = Vec3::new(to_player.x, 0.0, to_player.z);
            if flat.length_squared() > 1.0e-4 {
                transform.rotation = glam::Quat::from_rotation_arc(Vec3::Z, flat.normalize());
            }

            ai.aim_warmup += dt;
            if invulnerable || ai.aim_warmup < AIM_WARMUP_SECONDS || ai.fire_cooldown > 0.0 {
                continue;
            }
            ai.fire_cooldown = FIRE_INTERVAL;

            // Accuracy falls off with range: ~90% point blank, ~35% at max.
            let accuracy = (1.0 - distance / ENGAGE_RANGE).clamp(0.0, 1.0) * 0.55 + 0.35;
            // Deterministic per-shot jitter from positions (no RNG in systems).
            let roll = (muzzle.dot(Vec3::new(12.9898, 78.233, 37.719)).sin() * 43_758.547)
                .fract()
                .abs();
            let hit = roll < accuracy;
            if hit {
                let falloff = (1.0 - distance / ENGAGE_RANGE * 0.5).clamp(0.4, 1.0);
                total_damage += HIT_DAMAGE * falloff;
            }
            shots.push((muzzle, hit));
        }
    }

    // Tracer + muzzle flash feedback for every shot fired (hit or miss).
    for (muzzle, _) in &shots {
        spawn_tracer(world, resources, *muzzle, player_position);
    }
    if !shots.is_empty() {
        if let Some(mut flashes) = resources.get_mut::<super::bot_weapon::BotShotFlashes>() {
            for (muzzle, _) in &shots {
                // Shooter pose: muzzle minus the hand offset, facing player.
                let base = *muzzle - Vec3::Y * MUZZLE_HEIGHT;
                let to_player = player_position - base;
                let flat = Vec3::new(to_player.x, 0.0, to_player.z);
                let rotation = if flat.length_squared() > 1.0e-4 {
                    glam::Quat::from_rotation_arc(Vec3::Z, flat.normalize())
                } else {
                    glam::Quat::IDENTITY
                };
                flashes.0.push((base, rotation));
            }
        }
    }

    if total_damage > 0.0 {
        apply_player_damage(resources, total_damage);
    }
}

fn apply_player_damage(resources: &Resources, damage: f32) {
    let died = {
        let mut health = resources.expect_mut::<PlayerHealth>();
        if health.invulnerable > 0.0 {
            return;
        }
        health.current = (health.current - damage).max(0.0);
        health.hurt_flash = 1.0;
        health.current <= 0.0
    };
    if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
        if let Some(ref mut audio) = *audio {
            audio.play_hurt();
        }
    }
    if died {
        respawn_player(resources);
    }
}

/// Teleport the player back to the scene spawn, restore health, and grant a
/// short grace period.
fn respawn_player(resources: &Resources) {
    let spawn = resources.expect::<crate::game::Player>().spawn_position();
    let player_body = resources.expect::<PlayerBody>().0;
    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        physics.teleport_body(player_body, spawn);
    }
    {
        let mut player = resources.expect_mut::<crate::game::Player>();
        player.reset_motion();
    }
    let deaths = {
        let mut health = resources.expect_mut::<PlayerHealth>();
        health.current = health.max;
        health.invulnerable = RESPAWN_GRACE;
        health.deaths += 1;
        health.deaths
    };
    if let Some(mut toast) = resources.get_mut::<Toast>() {
        toast.show(format!("你被击倒了，已在出生点重生（第 {deaths} 次）"), 3.0);
    }
}

/// Decay the hurt flash with real time so it fades even while paused.
pub fn update(_world: &mut EngineWorld, resources: &Resources) {
    let real_dt = resources
        .get::<Time>()
        .map(|t| t.real_delta_seconds())
        .unwrap_or(0.0);
    if let Some(mut health) = resources.get_mut::<PlayerHealth>() {
        health.hurt_flash = (health.hurt_flash - real_dt * 2.2).max(0.0);
    }
}

/// A brief glowing streak from the enemy muzzle toward the player: a stretched
/// thin box oriented along the shot, auto-despawned via `TimedEffect`.
fn spawn_tracer(world: &mut EngineWorld, resources: &Resources, from: Vec3, to: Vec3) {
    let Some(assets) = resources.get::<super::WeaponFeedbackAssets>().map(|a| *a) else {
        return;
    };
    let offset = to - from;
    let length = offset.length();
    if length < 0.5 {
        return;
    }
    let direction = offset / length;
    let center = from + offset * 0.5;
    let rotation = glam::Quat::from_rotation_arc(Vec3::Z, direction);
    let tracer = world.spawn();
    world.add_component(
        tracer,
        Transform::new(center, rotation, Vec3::new(0.03, 0.03, length)),
    );
    world.add_component(
        tracer,
        crate::renderer::RenderMesh {
            mesh: assets.impact_mesh,
            material: assets.muzzle_flash_material,
        },
    );
    world.add_component(tracer, super::TimedEffect { remaining: 0.07 });
}
