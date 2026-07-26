use crate::core::{EngineWorld, Resources};
use std::collections::HashMap;
use std::sync::Arc;
use winit::event::Event;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorGrabMode, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Startup,
    PreUpdate,
    Update,
    FixedUpdate,
    LateUpdate,
    PreRender,
    Render,
    PostRender,
}

pub type System = Box<dyn FnMut(&mut EngineWorld, &Resources)>;

pub struct Schedule {
    systems: HashMap<Stage, Vec<System>>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: HashMap::new(),
        }
    }

    pub fn add_system<F>(&mut self, stage: Stage, system: F)
    where
        F: FnMut(&mut EngineWorld, &Resources) + 'static,
    {
        self.systems
            .entry(stage)
            .or_default()
            .push(Box::new(system));
    }

    pub fn run_stage(&mut self, stage: Stage, world: &mut EngineWorld, resources: &Resources) {
        if let Some(systems) = self.systems.get_mut(&stage) {
            for system in systems.iter_mut() {
                system(world, resources);
            }
        }
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to switch (or reload) the active scene. Consumed by `App::update`
/// at the start of the next frame, outside any system borrow.
#[derive(Default)]
pub struct SceneChangeRequest(pub Option<std::path::PathBuf>);

/// Set by the pause menu's quit button; polled by the event loop.
#[derive(Default, Clone, Copy)]
pub struct QuitRequest(pub bool);

/// Path of the scene currently loaded (for the menu's "reset" action).
pub struct CurrentScenePath(pub std::path::PathBuf);

pub struct App {
    pub window: Arc<Window>,
    pub world: EngineWorld,
    pub resources: Resources,
    schedule: Schedule,
}

impl App {
    pub async fn new(
        window: Arc<Window>,
        scene_path: Option<std::path::PathBuf>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = EngineWorld::new();
        let resources = Resources::new();

        let startup_scene = scene_path.clone().unwrap_or_else(|| {
            std::path::PathBuf::from(crate::game::systems::setup::DEFAULT_SCENE_PATH)
        });
        resources.insert(crate::game::systems::setup::RequestedScenePath(scene_path));
        resources.insert(CurrentScenePath(startup_scene));
        resources.insert(SceneChangeRequest::default());
        resources.insert(QuitRequest::default());
        resources.insert(crate::game::scene::SceneLibrary::scan());
        resources.insert(crate::core::UserConfig::load());
        resources.insert(crate::input::InputState::new());
        resources.insert(crate::core::Time::new());
        resources.insert(crate::renderer::Camera::default());
        resources.insert(crate::asset::AssetManager::new());
        resources.insert(crate::physics::PhysicsWorld::default());
        resources.insert(crate::game::Player::new());

        let renderer = crate::renderer::Renderer::new(Arc::clone(&window)).await?;
        let debug_ui = crate::renderer::debug_ui::DebugUi::new(
            &window,
            &renderer.device,
            renderer.surface_config.format,
        );
        resources.insert(std::cell::RefCell::new(debug_ui));
        let debug_line_renderer = crate::renderer::DebugLineRenderer::new(
            &renderer.device,
            renderer.surface_config.format,
        );
        let sky_renderer =
            crate::renderer::SkyRenderer::new(&renderer.device, wgpu::TextureFormat::Rgba16Float);
        resources.insert(renderer);
        resources.insert(debug_line_renderer);
        resources.insert(sky_renderer);

        let mut schedule = Schedule::new();
        crate::game::systems::setup::register(&mut schedule);

        schedule.run_stage(Stage::Startup, &mut world, &resources);
        apply_cursor_lock(&window, true);

        // Now register per-frame systems.
        crate::game::systems::register_default_systems(&mut schedule);

        Ok(Self {
            window,
            world,
            resources,
            schedule,
        })
    }

    pub fn handle_event(&mut self, event: &Event<()>, active_event_loop: &ActiveEventLoop) {
        use winit::event::{DeviceEvent, ElementState, MouseScrollDelta, WindowEvent};
        use winit::keyboard::PhysicalKey;

        match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                let mouse_locked = self
                    .resources
                    .get::<crate::game::systems::MouseLocked>()
                    .map(|m| m.0)
                    .unwrap_or(true);
                let menu_open = self
                    .resources
                    .get::<crate::game::systems::MenuState>()
                    .map(|m| m.0.open)
                    .unwrap_or(false);
                let focused = self
                    .resources
                    .get::<crate::input::InputState>()
                    .map(|input| input.window_focused)
                    .unwrap_or(true);
                if mouse_locked && !menu_open && focused {
                    let mut input = self.resources.expect_mut::<crate::input::InputState>();
                    input.process_mouse_motion(glam::Vec2::new(delta.0 as f32, delta.1 as f32));
                }
            }
            Event::WindowEvent { event, .. } => {
                // The debug panel gets first refusal on window events while
                // open (its text fields / sliders capture mouse + keys).
                let consumed = self
                    .resources
                    .get::<std::cell::RefCell<crate::renderer::debug_ui::DebugUi>>()
                    .map(|ui| ui.borrow_mut().on_window_event(&self.window, event))
                    .unwrap_or(false);
                if consumed {
                    return;
                }
                match event {
                    WindowEvent::CloseRequested => {
                        active_event_loop.exit();
                    }
                    WindowEvent::Resized(size) => {
                        if let Some(mut renderer) =
                            self.resources.get_mut::<crate::renderer::Renderer>()
                        {
                            renderer.resize(size.width, size.height);
                        }
                        if let Some(mut camera) =
                            self.resources.get_mut::<crate::renderer::Camera>()
                        {
                            camera.set_aspect(size.width as f32, size.height as f32);
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        if let PhysicalKey::Code(winit::keyboard::KeyCode::F1) = event.physical_key
                        {
                            if event.state == ElementState::Pressed {
                                let open = {
                                    let mut panel = self
                                    .resources
                                    .expect_mut::<crate::game::systems::debug_panel::DebugPanelState>();
                                    panel.open = !panel.open;
                                    panel.open
                                };
                                if let Some(ui) = self
                                    .resources
                                    .get::<std::cell::RefCell<crate::renderer::debug_ui::DebugUi>>()
                                {
                                    ui.borrow_mut().enabled = open;
                                }
                                // Free the cursor while the panel is open.
                                let mut mouse_locked =
                                    self.resources
                                        .expect_mut::<crate::game::systems::MouseLocked>();
                                mouse_locked.0 = !open
                                    && !self
                                        .resources
                                        .get::<crate::game::systems::MenuState>()
                                        .map(|m| m.0.open)
                                        .unwrap_or(false);
                                let locked = mouse_locked.0;
                                apply_cursor_lock(&self.window, locked);
                                return;
                            }
                        }
                        let mut input = self.resources.expect_mut::<crate::input::InputState>();
                        if let PhysicalKey::Code(keycode) = event.physical_key {
                            input.process_key(keycode, event.state);
                            if keycode == winit::keyboard::KeyCode::Escape
                                && event.state == ElementState::Pressed
                            {
                                drop(input);
                                let menu_open = {
                                    let mut menu = self
                                        .resources
                                        .expect_mut::<crate::game::systems::MenuState>();
                                    menu.0.open = !menu.0.open;
                                    menu.0.open
                                };
                                let mut mouse_locked =
                                    self.resources
                                        .expect_mut::<crate::game::systems::MouseLocked>();
                                mouse_locked.0 = !menu_open;
                                let locked = mouse_locked.0;
                                apply_cursor_lock(&self.window, locked);
                            }
                        }
                    }
                    WindowEvent::Focused(focused) => {
                        if let Some(mut input) =
                            self.resources.get_mut::<crate::input::InputState>()
                        {
                            input.window_focused = *focused;
                            if !*focused {
                                input.clear_transient_and_held_input();
                            }
                        }
                        if *focused {
                            let menu_open = self
                                .resources
                                .get::<crate::game::systems::MenuState>()
                                .map(|m| m.0.open)
                                .unwrap_or(false);
                            if !menu_open {
                                if let Some(mut mouse_locked) =
                                    self.resources
                                        .get_mut::<crate::game::systems::MouseLocked>()
                                {
                                    mouse_locked.0 = true;
                                }
                                apply_cursor_lock(&self.window, true);
                            }
                        } else {
                            apply_cursor_lock(&self.window, false);
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let mut input = self.resources.expect_mut::<crate::input::InputState>();
                        input.process_cursor_position(glam::Vec2::new(
                            position.x as f32,
                            position.y as f32,
                        ));
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        let mut input = self.resources.expect_mut::<crate::input::InputState>();
                        input.process_mouse_button(*button, *state);
                        if *state == ElementState::Pressed {
                            drop(input);
                            let menu_open = self
                                .resources
                                .get::<crate::game::systems::MenuState>()
                                .map(|m| m.0.open)
                                .unwrap_or(false);
                            let wants_lock = self
                                .resources
                                .get::<crate::game::systems::MouseLocked>()
                                .map(|m| m.0)
                                .unwrap_or(true);
                            if wants_lock && !menu_open {
                                apply_cursor_lock(&self.window, true);
                            }
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let mut input = self.resources.expect_mut::<crate::input::InputState>();
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(_, y) => *y,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                        };
                        input.process_mouse_scroll(scroll);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// True when the pause menu requested an application exit.
    pub fn quit_requested(&self) -> bool {
        self.resources
            .get::<QuitRequest>()
            .map(|q| q.0)
            .unwrap_or(false)
    }

    /// Tear down the live world and rebuild it from `path`. Everything the
    /// startup stage inserts is replaced wholesale; the GPU-side material
    /// bind-group cache is cleared because asset ids restart from 1.
    fn reload_scene(&mut self, path: std::path::PathBuf) {
        log::info!("Switching scene to {}", path.display());
        self.world.ecs.clear();
        self.resources
            .insert(crate::physics::PhysicsWorld::default());
        self.resources.insert(crate::asset::AssetManager::new());
        self.resources.insert(crate::game::Player::new());
        {
            let renderer = self.resources.expect::<crate::renderer::Renderer>();
            renderer.material_bind_groups.borrow_mut().clear();
        }
        self.resources
            .insert(crate::game::systems::setup::RequestedScenePath(Some(
                path.clone(),
            )));
        self.resources.insert(CurrentScenePath(path));
        self.resources
            .insert(crate::game::systems::PlayerHealth::default());
        self.schedule
            .run_stage(Stage::Startup, &mut self.world, &self.resources);
        // Close the menu and re-enter FPS mouse capture.
        if let Some(mut menu) = self.resources.get_mut::<crate::game::systems::MenuState>() {
            menu.0.open = false;
        }
        if let Some(mut locked) = self
            .resources
            .get_mut::<crate::game::systems::MouseLocked>()
        {
            locked.0 = true;
        }
        apply_cursor_lock(&self.window, true);
    }

    pub fn update(&mut self) {
        let pending_scene = self
            .resources
            .get_mut::<SceneChangeRequest>()
            .and_then(|mut request| request.0.take());
        if let Some(path) = pending_scene {
            self.reload_scene(path);
        }

        if let Some(mut time) = self.resources.get_mut::<crate::core::Time>() {
            time.update();
        }

        self.schedule
            .run_stage(Stage::PreUpdate, &mut self.world, &self.resources);
        self.schedule
            .run_stage(Stage::Update, &mut self.world, &self.resources);

        // Consume every accumulated fixed tick, with a safety cap to avoid a
        // long frame causing an unbounded catch-up spiral. The timestep fed to
        // Rapier remains constant; time_scale only controls tick frequency.
        let mut fixed_steps = 0;
        for _ in 0..8 {
            let should_fixed = self
                .resources
                .get_mut::<crate::core::Time>()
                .map(|mut t| t.should_fixed_update())
                .unwrap_or(false);
            if !should_fixed {
                break;
            }
            self.schedule
                .run_stage(Stage::FixedUpdate, &mut self.world, &self.resources);
            fixed_steps += 1;
        }
        if fixed_steps == 8 {
            if let Some(mut time) = self.resources.get_mut::<crate::core::Time>() {
                time.discard_fixed_update_backlog();
            }
        }

        self.schedule
            .run_stage(Stage::LateUpdate, &mut self.world, &self.resources);
        self.schedule
            .run_stage(Stage::PreRender, &mut self.world, &self.resources);
    }

    pub fn render(&mut self) {
        self.schedule
            .run_stage(Stage::Render, &mut self.world, &self.resources);
        self.schedule
            .run_stage(Stage::PostRender, &mut self.world, &self.resources);
    }
}

fn apply_cursor_lock(window: &Window, locked: bool) {
    window.set_cursor_visible(!locked);
    let cursor_mode = if locked {
        CursorGrabMode::Confined
    } else {
        CursorGrabMode::None
    };
    let _ = window.set_cursor_grab(cursor_mode);
}
