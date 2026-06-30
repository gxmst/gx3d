use crate::core::{EngineWorld, Resources};
use crate::game::{Language, MenuAction, RESOLUTION_OPTIONS};
use crate::renderer::Renderer;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::MouseButton;

use super::MenuState;

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    let (clicked, cursor) = {
        let input = resources
            .get::<crate::input::InputState>()
            .expect("Input missing");
        (
            input.is_mouse_just_pressed(MouseButton::Left),
            input.mouse_position,
        )
    };

    if !clicked {
        return;
    }

    let (width, height) = {
        let renderer = resources.get::<Renderer>().expect("Renderer missing");
        (
            renderer.surface_config.width as f32,
            renderer.surface_config.height as f32,
        )
    };

    let action = {
        let menu = resources.get::<MenuState>().expect("MenuState missing");
        if !menu.0.open {
            return;
        }
        menu.0.hit_test(width, height, cursor)
    };

    match action {
        Some(MenuAction::SetResolution(index)) => {
            let option = RESOLUTION_OPTIONS[index];
            {
                let mut menu = resources.get_mut::<MenuState>().expect("MenuState missing");
                menu.0.selected_resolution = index;
            }
            let renderer = resources.get::<Renderer>().expect("Renderer missing");
            let requested = PhysicalSize::new(option.width, option.height);
            let _ = renderer.window.request_inner_size(requested);
            center_window(&renderer.window, requested);
        }
        Some(MenuAction::SetLanguage(Language::SimplifiedChinese)) => {
            let mut menu = resources.get_mut::<MenuState>().expect("MenuState missing");
            menu.0.language = Language::SimplifiedChinese;
        }
        None => {}
    }
}

fn center_window(window: &winit::window::Window, size: PhysicalSize<u32>) {
    let Some(monitor) = window.primary_monitor() else {
        return;
    };
    let monitor_size = monitor.size();
    let monitor_position = monitor.position();
    let x = monitor_position.x + (monitor_size.width as i32 - size.width as i32) / 2;
    let y = monitor_position.y + (monitor_size.height as i32 - size.height as i32) / 2;
    window.set_outer_position(PhysicalPosition::new(
        x.max(monitor_position.x),
        y.max(monitor_position.y),
    ));
}
