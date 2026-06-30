use glam::Vec2;
use gxengine::core::{EngineWorld, Resources, Schedule, Stage};
use gxengine::game::systems;
use gxengine::input::{ButtonState, InputState};
use winit::event::ElementState;
use winit::keyboard::KeyCode;

#[test]
fn input_is_available_until_end_of_frame() {
    let mut schedule = Schedule::new();
    schedule.add_system(Stage::Update, |_world, resources| {
        let input = resources.get::<InputState>().unwrap();
        assert!(input.is_key_just_pressed(KeyCode::KeyG));
        assert_eq!(input.mouse_delta, Vec2::new(4.0, -2.0));
    });
    schedule.add_system(Stage::PostRender, systems::input::system);

    let mut world = EngineWorld::new();
    let resources = Resources::new();
    let mut input = InputState::new();
    input.process_key(KeyCode::KeyG, ElementState::Pressed);
    input.process_mouse_motion(Vec2::new(4.0, -2.0));
    resources.insert(input);

    schedule.run_stage(Stage::Update, &mut world, &resources);
    schedule.run_stage(Stage::PostRender, &mut world, &resources);

    let input = resources.get::<InputState>().unwrap();
    assert_eq!(input.keys.get(&KeyCode::KeyG), Some(&ButtonState::Held));
    assert_eq!(input.mouse_delta, Vec2::ZERO);
}
