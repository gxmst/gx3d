use crate::core::{EngineWorld, Resources};
use crate::input::InputState;
use winit::keyboard::KeyCode;

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    if super::menu_open(resources) {
        return;
    }
    let toggles = {
        let input = resources.expect::<InputState>();
        (
            input.is_key_just_pressed(KeyCode::F3),
            input.is_key_just_pressed(KeyCode::F4),
            input.is_key_just_pressed(KeyCode::F5),
            input.is_key_just_pressed(KeyCode::F6),
        )
    };
    if toggles.0 || toggles.1 || toggles.2 || toggles.3 {
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
        if toggles.3 {
            state.impulses = !state.impulses;
        }
    }
}
