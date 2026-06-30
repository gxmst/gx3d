use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::{EnemyAI, Player, Weapon, WeaponModel};
use crate::physics::{PhysicsBody, PhysicsWorld, Ray};
use crate::renderer::{Camera, RenderMesh};
use glam::{Quat, Vec3};

use super::{MenuState, MuzzleFlashTimer, TimedEffect, WeaponFeedbackAssets};

pub fn update_system(world: &mut EngineWorld, resources: &Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds().min(0.05))
        .unwrap_or(0.0);

    {
        let mut timer = resources
            .get_mut::<MuzzleFlashTimer>()
            .expect("MuzzleFlashTimer missing");
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
            .get::<crate::input::InputState>()
            .expect("Input missing")
            .clone();
        let mut weapon = resources.get_mut::<Weapon>().expect("Weapon missing");
        fired = weapon.update(dt, &input);
    }

    if fired {
        let (recoil, damage) = {
            let weapon_ref = resources.get::<Weapon>().expect("Weapon missing");
            (weapon_ref.get_recoil(), weapon_ref.damage)
        };
        {
            let mut player = resources.get_mut::<Player>().expect("Player missing");
            player.camera_controller.apply_recoil(recoil);
        }
        {
            let mut weapon_model = resources
                .get_mut::<WeaponModel>()
                .expect("WeaponModel missing");
            weapon_model.apply_recoil(Vec3::new(0.0, 0.005, -0.02));
        }
        {
            let mut timer = resources
                .get_mut::<MuzzleFlashTimer>()
                .expect("MuzzleFlashTimer missing");
            timer.0 = 0.06;
        }

        let (camera_pos, camera_forward) = {
            let camera = resources.get::<Camera>().expect("Camera missing");
            (camera.position, camera.forward())
        };
        {
            let ray = Ray::new(camera_pos, camera_forward, 100.0);
            let hit = {
                let physics = resources.get::<PhysicsWorld>().expect("Physics missing");
                physics.cast_ray(&ray)
            };
            if let Some(hit) = hit {
                if let Some(entity) = hit.entity {
                    if let Ok(mut ai) = world.ecs.get::<&mut EnemyAI>(entity) {
                        ai.take_damage(damage);
                        log::info!("Enemy hit! HP: {:.1}", ai.health);
                    }
                    if let Ok(body) = world.ecs.get::<&PhysicsBody>(entity) {
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
        .get_mut::<WeaponModel>()
        .expect("WeaponModel missing");
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

fn spawn_hit_feedback(world: &mut EngineWorld, resources: &Resources, point: Vec3, normal: Vec3) {
    let assets = resources
        .get::<WeaponFeedbackAssets>()
        .expect("WeaponFeedbackAssets missing");
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
    world.add_component(bullet_hole, TimedEffect { remaining: 22.0 });

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
