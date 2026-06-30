use crate::core::{EngineWorld, Resources, Time};
use crate::game::Player;
use crate::physics::PhysicsWorld;
use crate::renderer::Camera;
use glam::Vec3;
use winit::keyboard::KeyCode;

use super::PlayerBody;

pub fn input_system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
        let player_body = resources.get::<PlayerBody>().map(|body| body.0);
        if let (Some(player_body), Some(mut physics)) =
            (player_body, resources.get_mut::<PhysicsWorld>())
        {
            physics.set_body_horizontal_velocity(player_body, Vec3::ZERO);
        }
        return;
    }

    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds().min(0.05))
        .unwrap_or(0.0);

    let (forward, right) = {
        let camera = resources.expect::<Camera>();
        (camera.forward(), camera.right())
    };

    let (mouse_delta, wants_forward, wants_back, wants_right, wants_left, wants_sprint, wants_jump) = {
        let input = resources
            .expect::<crate::input::InputState>();
        (
            input.mouse_delta,
            input.is_key_pressed(KeyCode::KeyW),
            input.is_key_pressed(KeyCode::KeyS),
            input.is_key_pressed(KeyCode::KeyD),
            input.is_key_pressed(KeyCode::KeyA),
            input.is_key_pressed(KeyCode::ShiftLeft),
            input.is_key_pressed(KeyCode::Space),
        )
    };

    {
        let mut player = resources.remove::<Player>().expect("Player missing");
        {
            let mut camera = resources.expect_mut::<Camera>();
            player.camera_controller.update_camera_rotation(
                &mut camera,
                &crate::input::InputState {
                    mouse_delta,
                    ..Default::default()
                },
                dt,
            );
        }
        resources.insert(player);
    }

    let mut move_dir = Vec3::ZERO;
    if wants_forward {
        move_dir += forward;
    }
    if wants_back {
        move_dir -= forward;
    }
    if wants_right {
        move_dir += right;
    }
    if wants_left {
        move_dir -= right;
    }

    move_dir.y = 0.0;
    if move_dir.length_squared() > 0.0 {
        move_dir = move_dir.normalize();
    }

    let speed = if wants_sprint { 12.0 } else { 8.0 };
    let desired_velocity = move_dir * speed;

    let player_body = {
        let body = resources.expect::<PlayerBody>();
        body.0
    };
    {
        let mut physics = resources
            .expect_mut::<PhysicsWorld>();
        physics.set_body_horizontal_velocity(player_body, desired_velocity);

        if wants_jump {
            if let Some(vel) = physics.get_body_velocity(player_body) {
                if vel.y.abs() < 0.5 {
                    physics.apply_impulse(player_body, Vec3::new(0.0, 5.0, 0.0));
                }
            }
        }
    }
}

pub fn sync_system(_world: &mut EngineWorld, resources: &Resources) {
    let player_body = resources.expect::<PlayerBody>().0;

    if let Some(pos) = resources
        .get_mut::<PhysicsWorld>()
        .and_then(|p| p.get_body_position(player_body))
    {
        let mut camera = resources.expect_mut::<Camera>();
        camera.position = pos + Vec3::new(0.0, 0.8, 0.0);
    }
}
