use crate::core::{EngineWorld, Resources, Time};
use crate::game::Player;
use crate::input::InputState;
use crate::physics::PhysicsWorld;
use crate::renderer::Camera;
use glam::Vec3;
use winit::keyboard::KeyCode;

use super::{MenuState, PlayerBody, ViewMode, ViewModeState};

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
        return;
    }

    let (toggle, mouse_delta, forward, back, left, right, up, down, sprint) = {
        let input = resources.expect::<InputState>();
        (
            input.is_key_just_pressed(KeyCode::KeyV),
            input.mouse_delta,
            input.is_key_pressed(KeyCode::KeyW),
            input.is_key_pressed(KeyCode::KeyS),
            input.is_key_pressed(KeyCode::KeyA),
            input.is_key_pressed(KeyCode::KeyD),
            input.is_key_pressed(KeyCode::Space),
            input.is_key_pressed(KeyCode::ControlLeft),
            input.is_key_pressed(KeyCode::ShiftLeft),
        )
    };

    if toggle {
        let entered_god_mode = {
            let mut state = resources.expect_mut::<ViewModeState>();
            if state.god_mode_enabled {
                state.mode = match state.mode {
                    ViewMode::Fps => ViewMode::God,
                    ViewMode::God => ViewMode::Fps,
                };
                state.mode == ViewMode::God
            } else {
                false
            }
        };
        if entered_god_mode {
            let body = resources.expect::<PlayerBody>().0;
            resources
                .expect_mut::<PhysicsWorld>()
                .set_body_horizontal_velocity(body, Vec3::ZERO);
        }
    }

    let state = *resources.expect::<ViewModeState>();
    if state.mode != ViewMode::God {
        return;
    }

    let dt = resources.expect::<Time>().real_delta_seconds().min(0.05);
    {
        let mut player = resources.remove::<Player>().expect("Player missing");
        {
            let mut camera = resources.expect_mut::<Camera>();
            player.camera_controller.update_camera_rotation(
                &mut camera,
                &InputState {
                    mouse_delta,
                    ..Default::default()
                },
                dt,
            );
        }
        resources.insert(player);
    }

    let mut camera = resources.expect_mut::<Camera>();
    let mut direction = Vec3::ZERO;
    if forward {
        direction += camera.forward();
    }
    if back {
        direction -= camera.forward();
    }
    if right {
        direction += camera.right();
    }
    if left {
        direction -= camera.right();
    }
    if up {
        direction += Vec3::Y;
    }
    if down {
        direction -= Vec3::Y;
    }
    if direction.length_squared() > 0.0 {
        let speed = state.fly_speed * if sprint { 3.0 } else { 1.0 };
        camera.position += direction.normalize() * speed * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::systems::{MenuState, ViewModeState};

    #[test]
    fn god_camera_update_reinserts_player_after_camera_borrow_ends() {
        let resources = Resources::new();
        resources.insert(MenuState::default());
        resources.insert(InputState {
            mouse_delta: glam::Vec2::new(2.0, -1.0),
            ..Default::default()
        });
        resources.insert(Time::new());
        resources.insert(Camera::default());
        resources.insert(Player::new());
        resources.insert(ViewModeState {
            mode: ViewMode::God,
            ..Default::default()
        });

        let mut world = EngineWorld::new();
        system(&mut world, &resources);
        assert!(resources.contains::<Player>());
    }
}
