use crate::core::{EngineWorld, Resources, Transform};
use crate::input::InputState;
use crate::physics::sandbox::PhysicsSandbox;
use crate::physics::{PhysicsMaterial, PhysicsWorld, Ray};
use crate::renderer::Camera;
use glam::{Quat, Vec3};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        return;
    }
    // Buy menu captures input; spectators cannot manipulate the sandbox.
    let buy_open = resources
        .get::<super::buy_menu::BuyState>()
        .map(|buy| buy.open)
        .unwrap_or(false);
    let spectating = resources
        .get::<super::match_mode::MatchState>()
        .map(|state| state.player_spectating)
        .unwrap_or(false);
    if buy_open || spectating {
        return;
    }

    let (
        spawn_box,
        spawn_bouncy,
        spawn_heavy,
        pick_or_drop,
        toggle_freeze,
        throw_or_push,
        scroll_delta,
        delete_target,
    ) = {
        let input = resources.expect::<InputState>();
        (
            input.is_key_just_pressed(KeyCode::KeyG),
            // N spawns the bouncy ball; B is the buy menu everywhere.
            input.is_key_just_pressed(KeyCode::KeyN),
            input.is_key_just_pressed(KeyCode::KeyH),
            input.is_mouse_just_pressed(MouseButton::Middle)
                || input.is_key_just_pressed(KeyCode::KeyE),
            input.is_key_just_pressed(KeyCode::KeyX),
            input.is_key_just_pressed(KeyCode::KeyT),
            input.mouse_scroll,
            input.is_key_just_pressed(KeyCode::Delete),
        )
    };
    let (camera_pos, camera_forward) = {
        let camera = resources.expect::<Camera>();
        (camera.position, camera.forward())
    };

    let ray = Ray::new(camera_pos, camera_forward, 10.0);
    let player_body = resources.expect::<super::PlayerBody>().0;
    // Only probe for clear space when a spawn key was actually pressed; this
    // raycast is not needed on ordinary frames.
    let wants_spawn = spawn_box || spawn_bouncy || spawn_heavy;
    let spawn_position = wants_spawn
        .then(|| {
            let physics = resources.expect::<PhysicsWorld>();
            let distance = physics
                .cast_ray_excluding_body(&Ray::new(camera_pos, camera_forward, 3.8), player_body)
                .map(|hit| (hit.distance - 0.8).min(3.0))
                .unwrap_or(3.0);
            (distance >= 1.4).then_some(camera_pos + camera_forward * distance)
        })
        .flatten();
    let mut sandbox = resources.expect_mut::<PhysicsSandbox>();

    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();

        if spawn_box && spawn_position.is_none() {
            log::debug!("Not enough clear space in front of the camera to spawn a box");
        }
        if let (true, Some(spawn_position)) = (spawn_box, spawn_position) {
            let transform = Transform::new(spawn_position, Quat::IDENTITY, Vec3::ONE);
            let mesh = sandbox.cube_mesh;
            let material = sandbox.box_material;
            let entity =
                sandbox.spawn_dynamic_box(world, &mut physics, mesh, material, transform, 0.5, 1.0);
            world.add_component(entity, crate::core::Name("生成的木箱"));
            sandbox.track_runtime_prop(entity, world, &mut physics);
        }

        if spawn_bouncy && spawn_position.is_none() {
            log::debug!("Not enough clear space in front of the camera to spawn a ball");
        }
        if let (true, Some(spawn_position)) = (spawn_bouncy, spawn_position) {
            let transform = Transform::new(spawn_position, Quat::IDENTITY, Vec3::splat(0.8));
            let mesh = sandbox.sphere_mesh;
            let material = sandbox.bouncy_material;
            let entity = sandbox.spawn_dynamic_sphere(
                world,
                &mut physics,
                mesh,
                material,
                transform,
                0.4,
                0.6,
                PhysicsMaterial::RUBBER,
            );
            world.add_component(entity, crate::core::Name("高弹力球"));
            sandbox.track_runtime_prop(entity, world, &mut physics);
        }

        if spawn_heavy && spawn_position.is_none() {
            log::debug!("Not enough clear space in front of the camera to spawn a heavy prop");
        }
        if let (true, Some(spawn_position)) = (spawn_heavy, spawn_position) {
            let transform = Transform::new(spawn_position, Quat::IDENTITY, Vec3::splat(1.1));
            let mesh = sandbox.cube_mesh;
            let material = sandbox.heavy_material;
            let entity = sandbox.spawn_prop_with_physics_material(
                world,
                &mut physics,
                mesh,
                material,
                transform,
                crate::physics::PhysicsShape::Cuboid {
                    half_extents: Vec3::splat(0.55),
                },
                8.0,
                PhysicsMaterial::METAL,
            );
            world.add_component(entity, crate::core::Name("重型金属箱"));
            sandbox.track_runtime_prop(entity, world, &mut physics);
        }

        if pick_or_drop {
            if sandbox.held_body.is_some() {
                sandbox.drop_body();
            } else if let Some(body) =
                sandbox.ray_pick_excluding(world, &physics, &ray, player_body)
            {
                let distance = physics
                    .get_body_position(body)
                    .map(|p| (p - camera_pos).length())
                    .unwrap_or(3.0);
                sandbox.hold_body(body, distance);
            }
        }

        if toggle_freeze {
            let target = sandbox
                .held_body
                .or_else(|| sandbox.ray_pick_excluding(world, &physics, &ray, player_body));
            if let Some(body) = target {
                if physics.body_is_frozen(body) {
                    physics.unfreeze_body(body);
                } else {
                    physics.freeze_body(body);
                }
            }
        }

        if scroll_delta.abs() > f32::EPSILON {
            sandbox.adjust_hold_distance(scroll_delta);
        }

        if throw_or_push && !sandbox.throw_held(&mut physics, camera_forward, 18.0) {
            let push_ray = Ray::new(camera_pos, camera_forward, 12.0);
            if let Some(body) = sandbox.ray_pick_excluding(world, &physics, &push_ray, player_body)
            {
                physics.apply_impulse(body, camera_forward * 10.0);
            }
        }

        if delete_target {
            if let Some(hit) = physics.cast_ray_excluding_body(&ray, player_body) {
                if let Some(entity) = hit.entity {
                    if let Some(body) = world.get_component::<crate::physics::PhysicsBody>(entity) {
                        if physics.is_sandbox_manipulable(&body) {
                            let handle = body.rigid_body_handle;
                            if sandbox.held_body == Some(handle) {
                                sandbox.drop_body();
                            }
                            physics.remove_body(handle);
                            world.despawn(entity);
                            sandbox.forget_runtime_prop(entity);
                        }
                    }
                }
            }
        }
    }
}

/// Advance the held-prop servo on the same fixed clock as Rapier. This keeps
/// grabbing equally stiff at 30, 60, and 144 Hz render rates.
pub fn fixed_update(_world: &mut EngineWorld, resources: &Resources) {
    let (camera_pos, camera_forward) = {
        let camera = resources.expect::<Camera>();
        (camera.position, camera.forward())
    };
    let player_body = resources.expect::<super::PlayerBody>().0;
    let mut sandbox = resources.expect_mut::<PhysicsSandbox>();
    if sandbox.held_body.is_some() {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        sandbox.update_hold_excluding(&mut physics, camera_pos, camera_forward, Some(player_body));
    }
}
