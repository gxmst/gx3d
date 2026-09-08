//! egui debug panel plumbing: context, winit event bridge, wgpu renderer.
//!
//! This module owns only the *infrastructure* (context/renderer/screen state);
//! the panel's actual widgets live in `game::systems::debug_panel` so the
//! renderer layer stays ignorant of gameplay types.

use egui_wgpu::wgpu as egui_wgpu_types;
use std::sync::Arc;
use winit::window::Window;

pub struct DebugUi {
    pub context: egui::Context,
    winit_state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// Output of the last `begin_frame`/`end_frame` pair awaiting paint.
    pending: Option<egui::FullOutput>,
    pub enabled: bool,
}

impl DebugUi {
    pub fn new(window: &Arc<Window>, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        // Install CJK-capable fonts so panel labels can be Chinese.
        install_fonts(&context);
        let winit_state = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            None,
            None,
            None,
        );
        let renderer =
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Self {
            context,
            winit_state,
            renderer,
            pending: None,
            enabled: false,
        }
    }

    /// Feed a winit window event; returns true when egui consumed it (the
    /// game should then ignore it).
    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        if !self.enabled {
            return false;
        }
        self.winit_state.on_window_event(window, event).consumed
    }

    /// Run one egui frame: `build` adds the widgets to the root `Ui`.
    pub fn run(&mut self, window: &Window, mut build: impl FnMut(&mut egui::Ui)) {
        let input = self.winit_state.take_egui_input(window);
        let output = self.context.run_ui(input, |ui| build(ui));
        self.winit_state
            .handle_platform_output(window, output.platform_output.clone());
        self.pending = Some(output);
    }

    /// Paint the pending frame onto `target`.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        pixels_per_point: f32,
    ) {
        let Some(output) = self.pending.take() else {
            return;
        };
        let clipped = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        for (id, delta) in &output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width.max(1), height.max(1)],
            pixels_per_point,
        };
        self.renderer
            .update_buffers(device, queue, encoder, &clipped, &screen);
        {
            let pass = encoder.begin_render_pass(&egui_wgpu_types::RenderPassDescriptor {
                label: Some("Debug UI Pass"),
                color_attachments: &[Some(egui_wgpu_types::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: egui_wgpu_types::Operations {
                        load: egui_wgpu_types::LoadOp::Load,
                        store: egui_wgpu_types::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &clipped, &screen);
        }
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}

/// Reuse the overlay's system-font discovery so Chinese labels render.
fn install_fonts(context: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("cjk".to_string(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("cjk".to_string());
        }
        context.set_fonts(fonts);
        return;
    }
    log::warn!("No CJK font found for the debug panel; CJK labels will be boxes");
}
