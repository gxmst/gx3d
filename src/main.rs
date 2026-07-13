use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const DEFAULT_WINDOW_SIZE: PhysicalSize<u32> = PhysicalSize::new(1920, 1080);
const MAX_FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 144);

struct Runner {
    app: Option<gxengine::core::App>,
    next_frame: Instant,
}

impl Runner {
    fn new() -> Self {
        Self {
            app: None,
            next_frame: Instant::now(),
        }
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_some() {
            return;
        }
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("GxEngine - FPS Sandbox")
                .with_inner_size(DEFAULT_WINDOW_SIZE),
        ) {
            Ok(window) => window,
            Err(error) => {
                log::error!("Failed to create the window: {error}");
                eprintln!("GxEngine could not start: unable to open a window ({error}).");
                event_loop.exit();
                return;
            }
        };
        center_window(&window, DEFAULT_WINDOW_SIZE);
        let window = Arc::new(window);
        match pollster::block_on(gxengine::core::App::new(window)) {
            Ok(app) => self.app = Some(app),
            Err(error) => {
                log::error!("Failed to initialize the engine: {error}");
                eprintln!(
                    "GxEngine could not start: graphics initialization failed ({error}).\n\
                     Make sure your system has a GPU with current Vulkan, DirectX 12, or Metal drivers."
                );
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(app) = &mut self.app {
            app.handle_event(
                &winit::event::Event::WindowEvent { window_id, event },
                event_loop,
            );
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(app) = &mut self.app {
            app.handle_event(
                &winit::event::Event::DeviceEvent { device_id, event },
                event_loop,
            );
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(app) = &mut self.app else { return };
        let now = Instant::now();
        if now < self.next_frame {
            std::thread::sleep(self.next_frame - now);
        }
        self.next_frame = Instant::now() + MAX_FRAME_INTERVAL;
        app.update();
        app.render();
    }
}

fn main() {
    env_logger::init();
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log::error!("Failed to create the event loop: {error}");
            eprintln!("GxEngine could not start: unable to create an event loop ({error}).");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner::new();
    if let Err(error) = event_loop.run_app(&mut runner) {
        log::error!("Event loop exited with an error: {error}");
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
