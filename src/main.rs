use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

const DEFAULT_WINDOW_SIZE: PhysicalSize<u32> = PhysicalSize::new(1920, 1080);
const MAX_FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 144);

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = event_loop
        .create_window(
            Window::default_attributes()
                .with_title("GxEngine - FPS Sandbox")
                .with_inner_size(DEFAULT_WINDOW_SIZE),
        )
        .unwrap();
    center_window(&window, DEFAULT_WINDOW_SIZE);
    let window = Arc::new(window);

    let mut app = pollster::block_on(gxengine::core::App::new(Arc::clone(&window)))
        .expect("Failed to create app");
    let mut next_frame = Instant::now();

    event_loop
        .run(move |event, active_event_loop| {
            active_event_loop.set_control_flow(ControlFlow::Poll);
            app.handle_event(&event, active_event_loop);
            if let Event::AboutToWait = event {
                let now = Instant::now();
                if now < next_frame {
                    std::thread::sleep(next_frame - now);
                }
                next_frame = Instant::now() + MAX_FRAME_INTERVAL;
                app.update();
                app.render();
            }
        })
        .unwrap();
}

fn center_window(window: &Window, size: PhysicalSize<u32>) {
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
