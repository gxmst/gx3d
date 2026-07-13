use crate::core::{EngineWorld, Resources, Transform};
use crate::input::InputState;
use crate::physics::sandbox::PhysicsSandbox;
use crate::physics::{PhysicsMaterial, PhysicsWorld, Ray};
use crate::renderer::Camera;
use glam::{Quat, Vec3};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

pub fn system(world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
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
            input.is_key_just_pressed(KeyCode::KeyB),
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
    let mut sandbox = resources
        .remove::<PhysicsSandbox>()
        .expect("Sandbox missing");

    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();

        if spawn_box {
            let transform =
                Transform::new(camera_pos + camera_forward * 3.0, Quat::IDENTITY, Vec3::ONE);
            let mesh = sandbox.cube_mesh;
            let material = sandbox.box_material;
            sandbox.spawn_dynamic_box(world, &mut physics, mesh, material, transform, 0.5, 1.0);
        }

        if spawn_bouncy {
            let transform = Transform::new(
                camera_pos + camera_forward * 3.0,
                Quat::IDENTITY,
                Vec3::splat(0.8),
            );
            let mesh = sandbox.sphere_mesh;
            let material = sandbox.bouncy_material;
            sandbox.spawn_dynamic_sphere(
                world,
                &mut physics,
                mesh,
                material,
                transform,
                0.4,
                0.6,
                PhysicsMaterial::RUBBER,
            );
        }

        if spawn_heavy {
            let transform = Transform::new(
                camera_pos + camera_forward * 3.0,
                Quat::IDENTITY,
                Vec3::splat(1.1),
            );
            let mesh = sandbox.cube_mesh;
            let material = sandbox.heavy_material;
            sandbox.spawn_prop_with_physics_material(
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
            if let Some(body) = sandbox.held_body {
                if sandbox.is_frozen(&physics, body) {
                    sandbox.unfreeze(&mut physics, body);
                } else {
                    sandbox.freeze(&mut physics, body);
                }
            } else if let Some(body) =
                sandbox.ray_pick_excluding(world, &physics, &ray, player_body)
            {
                if sandbox.is_frozen(&physics, body) {
                    sandbox.unfreeze(&mut physics, body);
                } else {
                    sandbox.freeze(&mut physics, body);
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
                        if !body.is_static {
                            let handle = body.rigid_body_handle;
                            if sandbox.held_body == Some(handle) {
                                sandbox.drop_body();
                            }
                            physics.remove_body(handle);
                            world.despawn(entity);
                        }
                    }
                }
            }
        }

        if sandbox.held_body.is_some() {
            sandbox.update_hold(&mut physics, camera_pos, camera_forward);
        }
    }

    resources.insert(sandbox);
}
