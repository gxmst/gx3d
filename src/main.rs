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

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            log::error!("Failed to create the event loop: {e}");
            eprintln!("GxEngine could not start: unable to create a window event loop ({e}).");
            return;
        }
    };

    let window = match event_loop.create_window(
        Window::default_attributes()
            .with_title("GxEngine - FPS Sandbox")
            .with_inner_size(DEFAULT_WINDOW_SIZE),
    ) {
        Ok(window) => window,
        Err(e) => {
            log::error!("Failed to create the window: {e}");
            eprintln!("GxEngine could not start: unable to open a window ({e}).");
            return;
        }
    };
    center_window(&window, DEFAULT_WINDOW_SIZE);
    let window = Arc::new(window);

    let mut app = match pollster::block_on(gxengine::core::App::new(Arc::clone(&window))) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to initialize the engine: {e}");
            eprintln!(
                "GxEngine could not start: graphics initialization failed ({e}).\n\
                 Make sure your system has a GPU with up-to-date drivers that supports Vulkan, \
                 DirectX 12, or Metal."
            );
            return;
        }
    };
    let mut next_frame = Instant::now();

    let run_result = event_loop.run(move |event, active_event_loop| {
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
    });
    if let Err(e) = run_result {
        log::error!("Event loop exited with an error: {e}");
    }
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
