use crate::core::{EngineWorld, Resources, Time};
use crate::input::InputState;
use winit::keyboard::KeyCode;

const SPEEDS: [f32; 5] = [0.1, 0.25, 0.5, 1.0, 2.0];

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    if resources
        .get::<super::MenuState>()
        .map(|menu| menu.0.open)
        .unwrap_or(false)
    {
        return;
    }

    let (toggle_pause, slower, faster, single_step) = {
        let input = resources.expect::<InputState>();
        (
            input.is_key_just_pressed(KeyCode::KeyP),
            input.is_key_just_pressed(KeyCode::BracketLeft),
            input.is_key_just_pressed(KeyCode::BracketRight),
            input.is_key_just_pressed(KeyCode::Period),
        )
    };
    if !(toggle_pause || slower || faster || single_step) {
        return;
    }

    let mut time = resources.expect_mut::<Time>();
    if toggle_pause {
        let next_scale = if time.is_paused() { 1.0 } else { 0.0 };
        time.set_time_scale(next_scale);
    }
    if slower || faster {
        let current = if time.is_paused() {
            1.0
        } else {
            time.time_scale
        };
        let nearest = SPEEDS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (**a - current)
                    .abs()
                    .partial_cmp(&(**b - current).abs())
                    .unwrap()
            })
            .map(|(index, _)| index)
            .unwrap_or(3);
        let next = if slower {
            nearest.saturating_sub(1)
        } else {
            (nearest + 1).min(SPEEDS.len() - 1)
        };
        time.set_time_scale(SPEEDS[next]);
    }
    if single_step {
        time.set_time_scale(0.0);
        time.request_single_step();
    }
}
