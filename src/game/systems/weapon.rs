use crate::core::{EngineWorld, Resources, Time, Transform};
use crate::game::{EnemyAI, Explosive, Player, Weapon, WeaponModel};
use crate::physics::{PhysicsBody, PhysicsWorld, Ray};
use crate::renderer::{Camera, RenderMesh};
use glam::{Quat, Vec3};

use super::{
    EnemyFlashMaterial, HitFlash, HitMarkerTimer, MenuState, MuzzleFlashTimer, TimedEffect,
    WeaponFeedbackAssets,
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
    if let Some(mut timer) = resources.get_mut::<HitMarkerTimer>() {
        timer.0 = (timer.0 - real_dt).max(0.0);
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
                if let Some(mut timer) = resources.get_mut::<HitMarkerTimer>() {
                    timer.0 = if hit.entity.is_some() { 0.16 } else { 0.09 };
                }
                if let Some(entity) = hit.entity {
                    if world.ecs.get::<&Explosive>(entity).is_ok() {
                        detonate_explosive(world, resources, entity);
                    } else if world.ecs.get::<&EnemyAI>(entity).is_ok() {
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

fn detonate_explosive(world: &mut EngineWorld, resources: &Resources, entity: hecs::Entity) {
    let Some(explosive) = world.get_component::<Explosive>(entity) else {
        return;
    };
    let Some(origin) = world
        .get_component::<Transform>(entity)
        .map(|transform| transform.position)
    else {
        return;
    };
    let source_body = world
        .get_component::<PhysicsBody>(entity)
        .map(|body| body.rigid_body_handle);
    let radius = explosive.radius.max(0.1);
    let affected: Vec<_> = world
        .ecs
        .query::<(&Transform, &PhysicsBody)>()
        .iter()
        .map(|(transform, body)| (transform.position, body.rigid_body_handle))
        .collect();

    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        for (fallback_position, body_handle) in affected {
            if Some(body_handle) == source_body {
                continue;
            }
            let Some(body) = physics.rigid_body_set.get(body_handle) else {
                continue;
            };
            if !body.is_dynamic() {
                continue;
            }
            let position = physics
                .get_body_position(body_handle)
                .unwrap_or(fallback_position);
            let offset = position - origin;
            let distance = offset.length();
            if distance > radius
                || !explosion_reaches_body(&physics, origin, source_body, body_handle, position)
            {
                continue;
            }
            let direction = if distance > 0.01 {
                offset / distance
            } else {
                Vec3::Y
            };
            let falloff = (1.0 - distance / radius).max(0.12);
            let impulse =
                (direction + Vec3::Y * 0.28).normalize_or_zero() * explosive.impulse * falloff;
            physics.apply_impulse_at_point(body_handle, impulse, position);
        }
        if let Some(handle) = source_body {
            physics.remove_body(handle);
        }
    }

    let physics = resources.expect::<PhysicsWorld>();
    for (transform, body, ai) in world
        .ecs
        .query::<(&Transform, &PhysicsBody, &mut EnemyAI)>()
        .iter()
    {
        let distance = transform.position.distance(origin);
        if distance <= radius
            && explosion_reaches_body(
                &physics,
                origin,
                source_body,
                body.rigid_body_handle,
                transform.position,
            )
        {
            let falloff = 1.0 - distance / radius;
            ai.take_damage(140.0 * falloff.max(0.2));
            ai.stagger_timer = 0.35;
        }
    }
    drop(physics);
    world.despawn(entity);
    spawn_explosion_burst(world, resources, origin, radius);
    if let Some(mut audio) = resources.get_mut::<Option<crate::audio::AudioSystem>>() {
        if let Some(ref mut audio) = *audio {
            let _ = audio.play_sound(crate::audio::AudioSystem::create_explosion());
        }
    }
}

fn explosion_reaches_body(
    physics: &PhysicsWorld,
    origin: Vec3,
    source_body: Option<rapier3d::prelude::RigidBodyHandle>,
    target_body: rapier3d::prelude::RigidBodyHandle,
    target_position: Vec3,
) -> bool {
    let offset = target_position - origin;
    let distance = offset.length();
    if !distance.is_finite() {
        return false;
    }
    if distance <= 0.05 {
        return true;
    }

    let ray = Ray::new(origin, offset, distance + 0.08);
    let first_hit = match source_body {
        Some(source) => physics.cast_ray_excluding_body(&ray, source),
        None => physics.cast_ray(&ray),
    };
    first_hit.is_some_and(|hit| physics.collider_body(hit.collider_handle) == Some(target_body))
}

fn spawn_explosion_burst(
    world: &mut EngineWorld,
    resources: &Resources,
    origin: Vec3,
    radius: f32,
) {
    let assets = *resources.expect::<WeaponFeedbackAssets>();
    let directions = [
        Vec3::ZERO,
        Vec3::X,
        -Vec3::X,
        Vec3::Y,
        Vec3::Z,
        -Vec3::Z,
        Vec3::new(1.0, 0.6, 1.0).normalize(),
        Vec3::new(-1.0, 0.8, 1.0).normalize(),
        Vec3::new(1.0, 0.7, -1.0).normalize(),
        Vec3::new(-1.0, 0.5, -1.0).normalize(),
    ];
    for (index, direction) in directions.into_iter().enumerate() {
        let burst = world.spawn();
        let spread = radius.min(6.0) * (0.08 + index as f32 * 0.012);
        let scale = if index == 0 { 0.7 } else { 0.24 };
        world.add_component(
            burst,
            Transform::new(
                origin + direction * spread,
                Quat::IDENTITY,
                Vec3::splat(scale),
            ),
        );
        world.add_component(
            burst,
            RenderMesh {
                mesh: assets.impact_mesh,
                material: assets.impact_material,
            },
        );
        world.add_component(
            burst,
            TimedEffect {
                remaining: 0.14 + index as f32 * 0.018,
            },
        );
    }
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

#[cfg(test)]
mod tests {
    use super::detonate_explosive;
    use crate::core::{EngineWorld, Resources, Transform};
    use crate::game::systems::WeaponFeedbackAssets;
    use crate::game::Explosive;
    use crate::physics::{PhysicsBody, PhysicsShape, PhysicsWorld};
    use glam::Vec3;

    #[test]
    fn explosive_pushes_nearby_dynamic_body_and_removes_itself() {
        let mut world = EngineWorld::new();
        let resources = Resources::new();
        let mut physics = PhysicsWorld::default();

        let (source_body, source_collider) = physics.add_dynamic_body(
            Vec3::ZERO,
            PhysicsShape::Cylinder {
                radius: 0.3,
                half_height: 0.5,
            }
            .to_rapier_collider(),
            2.0,
        );
        let source = world.spawn();
        world.add_component(source, Transform::from_position(Vec3::ZERO));
        world.add_component(
            source,
            PhysicsBody::new(source_body, source_collider, false),
        );
        world.add_component(
            source,
            Explosive {
                radius: 5.0,
                impulse: 20.0,
            },
        );
        physics.register_entity(source_collider, source);

        let (target_body, target_collider) = physics.add_dynamic_body(
            Vec3::new(2.0, 0.0, 0.0),
            PhysicsShape::Sphere { radius: 0.4 }.to_rapier_collider(),
            1.0,
        );
        let target = world.spawn();
        world.add_component(target, Transform::from_position(Vec3::new(2.0, 0.0, 0.0)));
        world.add_component(
            target,
            PhysicsBody::new(target_body, target_collider, false),
        );
        physics.register_entity(target_collider, target);
        physics.step();

        resources.insert(physics);
        resources.insert(WeaponFeedbackAssets {
            bullet_hole_mesh: Default::default(),
            bullet_hole_material: Default::default(),
            impact_mesh: Default::default(),
            impact_material: Default::default(),
            muzzle_flash_mesh: Default::default(),
            muzzle_flash_material: Default::default(),
        });

        detonate_explosive(&mut world, &resources, source);

        assert!(!world.ecs.contains(source));
        let physics = resources.expect::<PhysicsWorld>();
        assert!(physics.get_body_velocity(target_body).unwrap().x > 0.0);
    }

    #[test]
    fn wall_blocks_explosion_impulse() {
        let mut world = EngineWorld::new();
        let resources = Resources::new();
        let mut physics = PhysicsWorld::new(Vec3::ZERO);

        let (source_body, source_collider) = physics.add_dynamic_body(
            Vec3::ZERO,
            PhysicsShape::Sphere { radius: 0.25 }.to_rapier_collider(),
            1.0,
        );
        let source = world.spawn();
        world.add_component(source, Transform::from_position(Vec3::ZERO));
        world.add_component(
            source,
            PhysicsBody::new(source_body, source_collider, false),
        );
        world.add_component(
            source,
            Explosive {
                radius: 5.0,
                impulse: 20.0,
            },
        );
        physics.register_entity(source_collider, source);

        physics.add_static_body(
            Vec3::new(1.0, 0.0, 0.0),
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(0.1, 2.0, 2.0),
            }
            .to_rapier_collider(),
        );
        let (target_body, target_collider) = physics.add_dynamic_body(
            Vec3::new(2.0, 0.0, 0.0),
            PhysicsShape::Sphere { radius: 0.35 }.to_rapier_collider(),
            1.0,
        );
        let target = world.spawn();
        world.add_component(target, Transform::from_position(Vec3::new(2.0, 0.0, 0.0)));
        world.add_component(
            target,
            PhysicsBody::new(target_body, target_collider, false),
        );
        physics.register_entity(target_collider, target);
        physics.step();

        resources.insert(physics);
        resources.insert(WeaponFeedbackAssets {
            bullet_hole_mesh: Default::default(),
            bullet_hole_material: Default::default(),
            impact_mesh: Default::default(),
            impact_material: Default::default(),
            muzzle_flash_mesh: Default::default(),
            muzzle_flash_material: Default::default(),
        });

        detonate_explosive(&mut world, &resources, source);

        let physics = resources.expect::<PhysicsWorld>();
        assert_eq!(physics.get_body_velocity(target_body), Some(Vec3::ZERO));
    }

    #[test]
    fn wall_blocks_explosion_damage_to_enemy() {
        let mut world = EngineWorld::new();
        let resources = Resources::new();
        let mut physics = PhysicsWorld::new(Vec3::ZERO);

        let (source_body, source_collider) = physics.add_dynamic_body(
            Vec3::ZERO,
            PhysicsShape::Sphere { radius: 0.25 }.to_rapier_collider(),
            1.0,
        );
        let source = world.spawn();
        world.add_component(source, Transform::from_position(Vec3::ZERO));
        world.add_component(
            source,
            PhysicsBody::new(source_body, source_collider, false),
        );
        world.add_component(
            source,
            Explosive {
                radius: 5.0,
                impulse: 20.0,
            },
        );
        physics.register_entity(source_collider, source);
        physics.add_static_body(
            Vec3::new(1.0, 0.0, 0.0),
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(0.1, 2.0, 2.0),
            }
            .to_rapier_collider(),
        );

        let enemy_position = Vec3::new(2.0, 0.0, 0.0);
        let (enemy_body, enemy_collider) = physics.add_kinematic_body(
            enemy_position,
            PhysicsShape::Capsule {
                radius: 0.4,
                half_height: 0.8,
            }
            .to_rapier_collider(),
        );
        let enemy = world.spawn();
        world.add_component(enemy, Transform::from_position(enemy_position));
        world.add_component(enemy, PhysicsBody::new(enemy_body, enemy_collider, false));
        world.add_component(enemy, crate::game::EnemyAI::new(Vec::new(), 2.0, 100.0));
        physics.register_entity(enemy_collider, enemy);
        physics.step();

        resources.insert(physics);
        resources.insert(WeaponFeedbackAssets {
            bullet_hole_mesh: Default::default(),
            bullet_hole_material: Default::default(),
            impact_mesh: Default::default(),
            impact_material: Default::default(),
            muzzle_flash_mesh: Default::default(),
            muzzle_flash_material: Default::default(),
        });

        detonate_explosive(&mut world, &resources, source);

        assert_eq!(
            world
                .get_component::<crate::game::EnemyAI>(enemy)
                .unwrap()
                .health,
            100.0
        );
    }
}
