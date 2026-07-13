use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::{EnemyAI, Player, Weapon, WeaponModel};
use crate::physics::{PhysicsBody, PhysicsWorld, Ray};
use crate::renderer::{Camera, RenderMesh};
use glam::{Quat, Vec3};

use super::{
    EnemyFlashMaterial, HitFlash, MenuState, MuzzleFlashTimer, TimedEffect, WeaponFeedbackAssets,
};

pub fn update_system(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds().min(0.05))
        .unwrap_or(0.0);
    let aiming = resources
        .get::<crate::input::InputState>()
        .map(|input| input.is_mouse_pressed(winit::event::MouseButton::Right))
        .unwrap_or(false);
    let real_dt = resources.expect::<Time>().real_delta_seconds().min(0.05);
    {
        let mut model = resources.expect_mut::<WeaponModel>();
        model.update_aim(aiming, real_dt);
    }

    {
        let mut timer = resources.expect_mut::<MuzzleFlashTimer>();
        if timer.0 > 0.0 {
            timer.0 -= dt;
        }
    }
    update_timed_effects(world, dt);

    let menu_open = resources
        .get::<MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false);
    if menu_open {
        update_weapon_model(resources, dt);
        return;
    }

    let fired: bool;
    {
        let input = resources.expect::<crate::input::InputState>().clone();
        let mut weapon = resources.expect_mut::<Weapon>();
        fired = weapon.update(dt, &input);
    }

    if fired {
        let (recoil, damage) = {
            let weapon_ref = resources.expect::<Weapon>();
            (
                weapon_ref.get_recoil() * if aiming { 0.62 } else { 1.0 },
                weapon_ref.damage,
            )
        };
        {
            let mut player = resources.expect_mut::<Player>();
            player.camera_controller.apply_recoil(recoil);
        }
        {
            let mut weapon_model = resources.expect_mut::<WeaponModel>();
            weapon_model.apply_recoil(Vec3::new(0.0, 0.005, -0.02));
        }
        {
            let mut timer = resources.expect_mut::<MuzzleFlashTimer>();
            timer.0 = 0.06;
        }

        let (camera_pos, camera_forward) = {
            let camera = resources.expect::<Camera>();
            (camera.position, camera.forward())
        };
        {
            let ray = Ray::new(camera_pos, camera_forward, 100.0);
            let hit = {
                let physics = resources.expect::<PhysicsWorld>();
                let player_body = resources.expect::<super::PlayerBody>().0;
                physics.cast_ray_excluding_body(&ray, player_body)
            };
            if let Some(hit) = hit {
                if let Some(entity) = hit.entity {
                    let is_enemy = world.ecs.get::<&EnemyAI>(entity).is_ok();
                    if is_enemy {
                        react_enemy_hit(
                            world,
                            resources,
                            entity,
                            damage,
                            camera_forward.normalize_or_zero(),
                        );
                    } else {
                        react_physics_hit(world, resources, &hit, camera_forward);
                    }
                } else {
                    // A collider can still have a valid parent body even when
                    // no ECS mapping exists. Keep physical hit feedback robust.
                    react_physics_hit(world, resources, &hit, camera_forward);
                }
                spawn_hit_feedback(world, resources, hit.point, hit.normal);
            }
        }

        if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
            if let Some(ref mut a) = *audio {
                let _ = a.play_sound(crate::audio::AudioSystem::create_gunshot());
            }
        }
    }

    update_weapon_model(resources, dt);
}

fn react_physics_hit(
    world: &EngineWorld,
    resources: &Resources,
    hit: &crate::physics::RaycastHit,
    shot_direction: Vec3,
) {
    let mapped_body = hit.entity.and_then(|entity| {
        world
            .ecs
            .get::<&PhysicsBody>(entity)
            .ok()
            .and_then(|body| (!body.is_static).then_some(body.rigid_body_handle))
    });
    let mut physics = resources.expect_mut::<PhysicsWorld>();
    let body_handle = mapped_body.or_else(|| physics.collider_body(hit.collider_handle));
    let Some(body_handle) = body_handle else {
        return;
    };
    let Some(body) = physics.rigid_body_set.get(body_handle) else {
        return;
    };
    if !body.is_dynamic() {
        return;
    }
    // Scale gently with mass so light balls visibly jump while heavy props
    // still acknowledge a hit without turning into rockets.
    let strength = 4.5 + body.mass().sqrt().min(4.0) * 1.4;
    physics.apply_impulse_at_point(
        body_handle,
        shot_direction.normalize_or_zero() * strength,
        hit.point,
    );
}

fn update_weapon_model(resources: &Resources, dt: f32) {
    let mut weapon_model = resources.expect_mut::<WeaponModel>();
    weapon_model.update(dt);
}

fn update_timed_effects(world: &mut EngineWorld, dt: f32) {
    let mut expired = Vec::new();
    for (entity, effect) in world.ecs.query::<(hecs::Entity, &mut TimedEffect)>().iter() {
        effect.remaining -= dt;
        if effect.remaining <= 0.0 {
            expired.push(entity);
        }
    }
    for entity in expired {
        world.despawn(entity);
    }
}

/// Apply the visible reaction when a shot hits an enemy: damage, a brief
/// stagger, a small knock along the shot direction, and a bright emissive
/// "flash" material swapped in for a moment. The flash is purely a material
/// handle swap (cheap, no renderer changes); the original material is stored on
/// a `HitFlash` component and restored later by `enemy::feedback_system`.
fn react_enemy_hit(
    world: &mut EngineWorld,
    resources: &Resources,
    entity: hecs::Entity,
    damage: f32,
    shot_dir: Vec3,
) {
    // Damage + stagger on the AI.
    if let Ok(mut ai) = world.ecs.get::<&mut EnemyAI>(entity) {
        ai.take_damage(damage);
        ai.stagger_timer = 0.18;
        log::info!("Enemy hit! HP: {:.1}", ai.health);
    }

    // Nudge the enemy along the shot direction so the hit reads as impact.
    // Enemies are kinematic (driven by Transform), so we move the Transform
    // directly rather than applying a physics impulse.
    if let Ok(mut transform) = world.ecs.get::<&mut Transform>(entity) {
        transform.position += shot_dir * 0.12;
    }

    // Swap in the bright flash material, remembering the original so it can be
    // restored. If the enemy is already flashing, just refresh the timer and
    // keep the stored original (don't capture the flash material as "original").
    let flash_material = resources.expect::<EnemyFlashMaterial>().0;
    let already_flashing = world.ecs.get::<&HitFlash>(entity).is_ok();
    if already_flashing {
        if let Ok(mut flash) = world.ecs.get::<&mut HitFlash>(entity) {
            flash.remaining = 0.12;
        }
    } else if let Some(original_material) = world
        .ecs
        .get::<&RenderMesh>(entity)
        .ok()
        .map(|rm| rm.material)
    {
        if let Ok(mut render_mesh) = world.ecs.get::<&mut RenderMesh>(entity) {
            render_mesh.material = flash_material;
        }
        world.add_component(
            entity,
            HitFlash {
                remaining: 0.12,
                original_material,
            },
        );
    }
}

fn spawn_hit_feedback(world: &mut EngineWorld, resources: &Resources, point: Vec3, normal: Vec3) {
    let assets = resources.expect::<WeaponFeedbackAssets>();
    let normal = normal.normalize_or_zero();
    let base_rotation = Quat::from_rotation_arc(Vec3::Y, normal);
    let random_roll = ((point.dot(Vec3::new(12.9898, 78.233, 37.719)).sin() * 43_758.547)
        .fract()
        .abs())
        * std::f32::consts::TAU;
    let rotation = base_rotation * Quat::from_rotation_y(random_roll);

    let bullet_hole = world.spawn();
    world.add_component(
        bullet_hole,
        Transform::new(
            point + normal * 0.012,
            rotation,
            Vec3::new(0.16, 0.006, 0.16),
        ),
    );
    world.add_component(
        bullet_hole,
        RenderMesh {
            mesh: assets.bullet_hole_mesh,
            material: assets.bullet_hole_material,
        },
    );
    world.add_component(bullet_hole, TimedEffect { remaining: 12.0 });

    let impact = world.spawn();
    world.add_component(
        impact,
        Transform::new(point + normal * 0.035, Quat::IDENTITY, Vec3::splat(0.09)),
    );
    world.add_component(
        impact,
        RenderMesh {
            mesh: assets.impact_mesh,
            material: assets.impact_material,
        },
    );
    world.add_component(impact, TimedEffect { remaining: 0.08 });
}
