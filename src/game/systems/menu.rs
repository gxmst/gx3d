use crate::core::{EngineWorld, Resources};
use crate::game::{Language, MenuAction, RESOLUTION_OPTIONS};
use crate::renderer::Renderer;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::MouseButton;

use super::MenuState;

pub fn system(_world: &mut EngineWorld, resources: &Resources) {
    let (clicked, cursor) = {
        let input = resources.expect::<crate::input::InputState>();
        (
            input.is_mouse_just_pressed(MouseButton::Left),
            input.mouse_position,
        )
    };

    if !clicked {
        return;
    }

    let (width, height) = {
        let renderer = resources.expect::<Renderer>();
        (
            renderer.surface_config.width as f32,
            renderer.surface_config.height as f32,
        )
    };

    let action = {
        let menu = resources.expect::<MenuState>();
        if !menu.0.open {
            return;
        }
        menu.0.hit_test(width, height, cursor)
    };

    match action {
        Some(MenuAction::SetResolution(index)) => {
            let Some(option) = RESOLUTION_OPTIONS.get(index).copied() else {
                return;
            };
            {
                let mut menu = resources.expect_mut::<MenuState>();
                menu.0.selected_resolution = index;
            }
            {
                let renderer = resources.expect::<Renderer>();
                let requested = PhysicalSize::new(option.width, option.height);
                let _ = renderer.window.request_inner_size(requested);
                center_window(&renderer.window, requested);
            }
            save_config(resources);
        }
        Some(MenuAction::SetLanguage(Language::SimplifiedChinese)) => {
            let mut menu = resources.expect_mut::<MenuState>();
            menu.0.language = Language::SimplifiedChinese;
        }
        Some(MenuAction::ToggleGodMode) => {
            let enabled = {
                let mut menu = resources.expect_mut::<MenuState>();
                menu.0.god_mode_enabled = !menu.0.god_mode_enabled;
                menu.0.god_mode_enabled
            };
            {
                let mut view = resources.expect_mut::<super::ViewModeState>();
                view.god_mode_enabled = enabled;
                if !enabled {
                    view.mode = super::ViewMode::Fps;
                }
            }
            save_config(resources);
        }
        Some(MenuAction::CycleWeather) => {
            let kind = {
                let mut menu = resources.expect_mut::<MenuState>();
                menu.0.weather = menu.0.weather.next();
                menu.0.weather
            };
            if let Some(mut weather) =
                resources.get_mut::<crate::game::systems::weather::WeatherState>()
            {
                weather.selected = kind;
            }
            save_config(resources);
        }
        Some(MenuAction::CycleSensitivity) => {
            let (old_index, new_index) = {
                let mut menu = resources.expect_mut::<MenuState>();
                let old = menu.0.sensitivity_index;
                menu.0.sensitivity_index =
                    (old + 1) % crate::core::config::SENSITIVITY_OPTIONS.len();
                (old, menu.0.sensitivity_index)
            };
            // Rescale the live sensitivity in place: divide out the old
            // multiplier, apply the new one.
            let old_mult = crate::core::config::SENSITIVITY_OPTIONS[old_index];
            let new_mult = crate::core::config::SENSITIVITY_OPTIONS[new_index];
            {
                let mut player = resources.expect_mut::<crate::game::Player>();
                player.camera_controller.mouse_sensitivity *= new_mult / old_mult;
            }
            save_config(resources);
        }
        Some(MenuAction::LoadScene(index)) => {
            let path = resources
                .expect::<crate::game::scene::SceneLibrary>()
                .0
                .get(index)
                .map(|entry| entry.path.clone());
            if let Some(path) = path {
                resources.expect_mut::<crate::core::SceneChangeRequest>().0 = Some(path);
            }
        }
        Some(MenuAction::ResetScene) => {
            let current = resources
                .expect::<crate::core::CurrentScenePath>()
                .0
                .clone();
            resources.expect_mut::<crate::core::SceneChangeRequest>().0 = Some(current);
        }
        Some(MenuAction::Resume) => {
            {
                let mut menu = resources.expect_mut::<MenuState>();
                menu.0.open = false;
            }
            let mut locked = resources.expect_mut::<super::MouseLocked>();
            locked.0 = true;
            let renderer = resources.expect::<Renderer>();
            apply_cursor_lock(&renderer.window, true);
        }
        Some(MenuAction::Quit) => {
            resources.expect_mut::<crate::core::QuitRequest>().0 = true;
        }
        None => {}
    }
}

/// Persist the current menu settings to config.json.
fn save_config(resources: &crate::core::Resources) {
    let menu = resources.expect::<MenuState>();
    let mut config = resources.expect_mut::<crate::core::UserConfig>();
    config.resolution_index = menu.0.selected_resolution;
    config.sensitivity_index = menu.0.sensitivity_index;
    config.god_mode_enabled = menu.0.god_mode_enabled;
    config.weather = menu.0.weather;
    config.save();
}

fn apply_cursor_lock(window: &winit::window::Window, locked: bool) {
    window.set_cursor_visible(!locked);
    let mode = if locked {
        winit::window::CursorGrabMode::Confined
    } else {
        winit::window::CursorGrabMode::None
    };
    let _ = window.set_cursor_grab(mode);
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
