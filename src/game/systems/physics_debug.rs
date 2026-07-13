use crate::core::{EngineWorld, Resources};
use crate::input::InputState;
use winit::keyboard::KeyCode;

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
        return;
    }
    let toggles = {
        let input = resources.expect::<InputState>();
        (
            input.is_key_just_pressed(KeyCode::F3),
            input.is_key_just_pressed(KeyCode::F4),
            input.is_key_just_pressed(KeyCode::F5),
        )
    };
    if toggles.0 || toggles.1 || toggles.2 {
        let mut state = resources.expect_mut::<super::PhysicsDebugState>();
        if toggles.0 {
            state.colliders = !state.colliders;
        }
        if toggles.1 {
            state.velocities = !state.velocities;
        }
        if toggles.2 {
            state.contacts = !state.contacts;
        }
    }
}
