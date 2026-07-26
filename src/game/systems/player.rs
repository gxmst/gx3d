use crate::core::{EngineWorld, Resources, Time};
use crate::game::Player;
use crate::physics::PhysicsWorld;
use crate::renderer::Camera;
use glam::Vec3;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use super::PlayerBody;

const PLAYER_EFFECTIVE_MASS: f32 = 75.0;
const MAX_STEP_HEIGHT: f32 = 0.36;
const GROUND_SNAP_DISTANCE: f32 = 0.24;

pub fn input_system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode == super::ViewMode::God)
        .unwrap_or(false)
    {
        return;
    }
    if super::menu_open(resources) {
        if let Some(mut player) = resources.get_mut::<Player>() {
            player.stop_movement();
        }
        return;
    }

    let real_dt = resources
        .get::<Time>()
        .map(|time| time.real_delta_seconds().min(0.05))
        .unwrap_or(0.0);

    let (forward, right) = {
        let camera = resources.expect::<Camera>();
        (camera.forward(), camera.right())
    };

    let (
        mouse_delta,
        wants_forward,
        wants_back,
        wants_right,
        wants_left,
        wants_sprint,
        wants_jump,
        aiming,
    ) = {
        let input = resources.expect::<crate::input::InputState>();
        (
            input.mouse_delta,
            input.is_key_pressed(KeyCode::KeyW),
            input.is_key_pressed(KeyCode::KeyS),
            input.is_key_pressed(KeyCode::KeyD),
            input.is_key_pressed(KeyCode::KeyA),
            input.is_key_pressed(KeyCode::ShiftLeft),
            input.is_key_just_pressed(KeyCode::Space),
            input.is_mouse_pressed(MouseButton::Right),
        )
    };

    {
        let mut camera = resources.expect_mut::<Camera>();
        let target_fov = if aiming { 48.0 } else { 70.0 };
        camera.fov += (target_fov - camera.fov) * (real_dt * 12.0).clamp(0.0, 1.0);
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

    {
        let mut player = resources.expect_mut::<Player>();
        {
            let mut camera = resources.expect_mut::<Camera>();
            player.camera_controller.update_camera_rotation(
                &mut camera,
                &crate::input::InputState {
                    mouse_delta: mouse_delta * if aiming { 0.55 } else { 1.0 },
                    ..Default::default()
                },
                real_dt,
            );
        }
        player.set_movement_input(move_dir, wants_sprint);
        if wants_jump {
            player.queue_jump();
        }
    }
}

/// Advance the player controller at the same fixed cadence as Rapier. Keeping
/// input sampling in `Update` and collision movement here avoids frame-rate
/// dependent speed and preserves short jump presses with a small input buffer.
pub fn fixed_update_system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode == super::ViewMode::God)
        .unwrap_or(false)
    {
        return;
    }

    let player_body = resources.expect::<PlayerBody>().0;
    let mut player = resources.expect_mut::<Player>();
    {
        let mut physics = resources.expect_mut::<PhysicsWorld>();
        let Some(position) = physics.get_body_position(player_body) else {
            return;
        };

        if player.needs_fall_recovery(position) {
            let recovery_position = player.recovery_position();
            if physics.teleport_body(player_body, recovery_position) {
                player.reset_motion();
            }
            return;
        }

        let dt = physics.integration_parameters.dt;
        let desired_translation = player.desired_translation(dt, physics.gravity.y);
        let controller = crate::physics::character_controller(
            MAX_STEP_HEIGHT,
            if player.is_rising() {
                None
            } else {
                Some(GROUND_SNAP_DISTANCE)
            },
        );

        match physics.move_kinematic_character(
            player_body,
            desired_translation,
            &controller,
            PLAYER_EFFECTIVE_MASS,
        ) {
            Some(movement) => player.finish_character_move(
                movement.grounded,
                movement.hit_ceiling,
                position + movement.translation,
            ),
            // Even when no movement was resolved this tick, grounded/coyote
            // state and the fall-recovery checkpoint must keep advancing.
            None => player.finish_character_move(false, false, position),
        }
    }
}

pub fn sync_system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::ViewModeState>()
        .map(|state| state.mode == super::ViewMode::God)
        .unwrap_or(false)
    {
        return;
    }
    let player_body = resources.expect::<PlayerBody>().0;
    let eye_height = resources.expect::<Player>().height * 0.5;

    if let Some(pos) = resources
        .get_mut::<PhysicsWorld>()
        .and_then(|p| p.get_body_position(player_body))
    {
        let mut camera = resources.expect_mut::<Camera>();
        camera.position = pos + Vec3::Y * eye_height;
    }
}
