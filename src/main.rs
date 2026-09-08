use std::path::PathBuf;
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
const USAGE: &str = "用法: gxengine [--scene <名字或路径>]\n  \
    名字会解析成 assets/scenes/<名字>.json，例如 --scene sandbox";

struct Runner {
    app: Option<gxengine::core::App>,
    next_frame: Instant,
    scene_path: Option<PathBuf>,
}

impl Runner {
    fn new(scene_path: Option<PathBuf>) -> Self {
        Self {
            app: None,
            next_frame: Instant::now(),
            scene_path,
        }
    }
}

/// Parse and validate CLI arguments up front so typos fail fast with a clear
/// message instead of being silently ignored during startup.
fn parse_scene_arg() -> Result<Option<PathBuf>, String> {
    let mut scene = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if arg == "--scene" {
            args.next()
                .ok_or_else(|| "--scene 需要一个场景名字或路径".to_string())?
        } else if let Some(value) = arg.strip_prefix("--scene=") {
            value.to_string()
        } else {
            return Err(format!("未知参数: {arg}"));
        };
        if value.is_empty() || value.starts_with('-') {
            return Err(format!("--scene 的值不合法: {value:?}"));
        }
        let path = PathBuf::from(&value);
        let path = if path.extension().is_some() || path.components().count() > 1 {
            path
        } else {
            std::path::Path::new("assets/scenes")
                .join(value)
                .with_extension("json")
        };
        if !path.is_file() {
            return Err(format!("场景文件不存在: {}", path.display()));
        }
        scene = Some(path);
    }
    Ok(scene)
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
        match pollster::block_on(gxengine::core::App::new(window, self.scene_path.take())) {
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(app) = &mut self.app else { return };
        let now = Instant::now();
        // Let winit wait for the deadline instead of sleeping on the event
        // loop thread: input events keep flowing during the wait and the cap
        // isn't quantized by the OS sleep granularity (~15.6 ms on Windows).
        if now < self.next_frame {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
            return;
        }
        self.next_frame = now + MAX_FRAME_INTERVAL;
        app.update();
        app.render();
        if app.quit_requested() {
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

fn main() {
    // Engine logs (scene fallbacks, missing assets, surface trouble) should be
    // visible by default; RUST_LOG still overrides.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("gxengine=info"))
        .init();
    let scene_path = match parse_scene_arg() {
        Ok(scene_path) => scene_path,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log::error!("Failed to create the event loop: {error}");
            eprintln!("GxEngine could not start: unable to create an event loop ({error}).");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut runner = Runner::new(scene_path);
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
