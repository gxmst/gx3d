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

    {
        let mut timer = resources
            .expect_mut::<MuzzleFlashTimer>();
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
        let input = resources
            .expect::<crate::input::InputState>()
            .clone();
        let mut weapon = resources.expect_mut::<Weapon>();
        fired = weapon.update(dt, &input);
    }

    if fired {
        let (recoil, damage) = {
            let weapon_ref = resources.expect::<Weapon>();
            (weapon_ref.get_recoil(), weapon_ref.damage)
        };
        {
            let mut player = resources.expect_mut::<Player>();
            player.camera_controller.apply_recoil(recoil);
        }
        {
            let mut weapon_model = resources
                .expect_mut::<WeaponModel>();
            weapon_model.apply_recoil(Vec3::new(0.0, 0.005, -0.02));
        }
        {
            let mut timer = resources
                .expect_mut::<MuzzleFlashTimer>();
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
                physics.cast_ray(&ray)
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
                    } else if let Ok(body) = world.ecs.get::<&PhysicsBody>(entity) {
                        if !body.is_static {
                            if let Some(mut physics) = resources.get_mut::<PhysicsWorld>() {
                                physics.apply_impulse(
                                    body.rigid_body_handle,
                                    camera_forward.normalize_or_zero() * 2.4,
                                );
                            }
                        }
                    }
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

fn update_weapon_model(resources: &Resources, dt: f32) {
    let mut weapon_model = resources
        .expect_mut::<WeaponModel>();
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
    let assets = resources
        .expect::<WeaponFeedbackAssets>();
    let normal = normal.normalize_or_zero();
    let rotation = Quat::from_rotation_arc(Vec3::Y, normal);

    let bullet_hole = world.spawn();
    world.add_component(
        bullet_hole,
        Transform::new(
            point + normal * 0.018,
            rotation,
            Vec3::new(0.11, 0.006, 0.11),
        ),
    );
    world.add_component(
        bullet_hole,
        RenderMesh {
            mesh: assets.bullet_hole_mesh,
            material: assets.bullet_hole_material,
        },
    );
    // Short-lived so impact marks read as feedback, not permanent litter.
    world.add_component(bullet_hole, TimedEffect { remaining: 3.0 });

    let impact = world.spawn();
    world.add_component(
        impact,
        Transform::new(point + normal * 0.04, Quat::IDENTITY, Vec3::splat(0.12)),
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
