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

    pub fn clear_stage(&mut self, stage: Stage) {
        self.systems.remove(&stage);
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

/// A convenience resource that stages can use to request a shutdown.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitRequested(pub bool);

pub struct App {
    pub window: Arc<Window>,
    pub world: EngineWorld,
    pub resources: Resources,
    schedule: Schedule,
}

impl App {
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut world = EngineWorld::new();
        let resources = Resources::new();

        resources.insert(crate::input::InputState::new());
        resources.insert(crate::core::Time::new());
        resources.insert(crate::renderer::Camera::default());
        resources.insert(crate::asset::AssetManager::new());
        resources.insert(crate::physics::PhysicsWorld::default());
        resources.insert(crate::game::Player::new());
        resources.insert(crate::core::ExitRequested(false));

        let renderer = crate::renderer::Renderer::new(Arc::clone(&window)).await?;
        resources.insert(renderer);

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
                    let mut input = self
                        .resources
                        .expect_mut::<crate::input::InputState>();
                    input.process_mouse_motion(glam::Vec2::new(delta.0 as f32, delta.1 as f32));
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    active_event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    if let Some(mut renderer) =
                        self.resources.get_mut::<crate::renderer::Renderer>()
                    {
                        renderer.resize(size.width, size.height);
                    }
                    if let Some(mut camera) = self.resources.get_mut::<crate::renderer::Camera>() {
                        camera.set_aspect(size.width as f32, size.height as f32);
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let mut input = self
                        .resources
                        .expect_mut::<crate::input::InputState>();
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
                            let mut mouse_locked = self
                                .resources
                                .expect_mut::<crate::game::systems::MouseLocked>();
                            mouse_locked.0 = !menu_open;
                            let locked = mouse_locked.0;
                            apply_cursor_lock(&self.window, locked);
                        }
                    }
                }
                WindowEvent::Focused(focused) => {
                    if let Some(mut input) = self.resources.get_mut::<crate::input::InputState>() {
                        input.window_focused = *focused;
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
                    let mut input = self
                        .resources
                        .expect_mut::<crate::input::InputState>();
                    input.process_cursor_position(glam::Vec2::new(
                        position.x as f32,
                        position.y as f32,
                    ));
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let mut input = self
                        .resources
                        .expect_mut::<crate::input::InputState>();
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
                    let mut input = self
                        .resources
                        .expect_mut::<crate::input::InputState>();
                    let scroll = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    input.process_mouse_scroll(scroll);
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn update(&mut self) {
        if let Some(mut time) = self.resources.get_mut::<crate::core::Time>() {
            time.update();
        }

        self.schedule
            .run_stage(Stage::PreUpdate, &mut self.world, &self.resources);
        self.schedule
            .run_stage(Stage::Update, &mut self.world, &self.resources);

        let should_fixed = self
            .resources
            .get_mut::<crate::core::Time>()
            .map(|mut t| t.should_fixed_update())
            .unwrap_or(false);
        if should_fixed {
            self.schedule
                .run_stage(Stage::FixedUpdate, &mut self.world, &self.resources);
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
