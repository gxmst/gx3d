mod font;
mod hud_text;
mod layout;
mod pause_text;
mod primitives;

use crate::game::{InteractionFocus, PauseMenu, PauseMenuLayout, UiRect};
use wgpu_text::{
    glyph_brush::{ab_glyph::FontArc, OwnedSection, OwnedText},
    BrushBuilder, TextBrush,
};

use font::load_ui_font;
use hud_text::{build_help_rectangles, build_help_text_sections, build_hud_text_sections};
use layout::{debug_badge_rects, HudLayout};
use pause_text::build_pause_text_sections;
use primitives::{
    add_button, add_card, add_crosshair, add_hit_marker, add_hud_panel, add_outline, add_panel,
    add_rect, rect_vertex_buffer_size, rect_vertex_capacity, reload_progress, OverlayVertex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudMode {
    Fps,
    God,
}

impl HudMode {
    fn short_label(self) -> &'static str {
        match self {
            Self::Fps => "FPS",
            Self::God => "GOD",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Fps => "第一人称模式",
            Self::God => "自由观察模式",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OverlayRuntimeInfo {
    pub mode: HudMode,
    pub fps: f32,
    pub time_scale: f32,
    pub paused: bool,
    pub debug_colliders: bool,
    pub debug_velocities: bool,
    pub debug_contacts: bool,
    pub debug_impulses: bool,
    pub camera_position: [f32; 3],
    pub light_count: usize,
    pub dynamic_body_count: usize,
    pub holding_object: bool,
    pub hold_distance: f32,
}

impl OverlayRuntimeInfo {
    fn time_label(self) -> String {
        if self.paused {
            "模拟已暂停".to_string()
        } else {
            format!("时间 {:.2}×", self.time_scale)
        }
    }

    fn enabled_debug_count(self) -> usize {
        [
            self.debug_colliders,
            self.debug_velocities,
            self.debug_contacts,
            self.debug_impulses,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HudState<'a> {
    pub focus: &'a InteractionFocus,
    pub weapon_name: &'a str,
    pub current_ammo: u32,
    pub max_ammo: u32,
    pub is_reloading: bool,
    pub reload_timer: f32,
    pub reload_duration: f32,
    pub aiming: bool,
    pub show_help: bool,
    pub hit_marker_seconds: f32,
    /// Highlighted onboarding hint until the player opens the F help once.
    pub first_time: bool,
    /// Transient notification text (scene load failures, respawns).
    pub toast: Option<&'a str>,
    /// Match scoreboard line (present only in match-mode scenes).
    pub match_line: Option<&'a str>,
    /// Buy menu rows when open (rendered as a centered panel).
    pub buy_lines: Option<&'a [String]>,
    /// Scoreboard rows while Tab is held (match mode).
    pub scoreboard: Option<&'a [String]>,
    /// Player money (match mode only).
    pub money: Option<u32>,
    pub health: f32,
    pub max_health: f32,
    /// 0..1 red damage flash drawn over the screen edges.
    pub hurt_flash: f32,
    pub runtime: OverlayRuntimeInfo,
}

pub struct OverlayRenderer {
    rect_pipeline: wgpu::RenderPipeline,
    rect_vertex_buffer: wgpu::Buffer,
    rect_vertex_capacity: usize,
    rect_vertex_count: u32,
    text_brush: Option<TextBrush<FontArc>>,
}

impl OverlayRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Overlay Pipeline Layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Rect Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[OverlayVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let rect_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Overlay Rect Vertex Buffer"),
            size: std::mem::size_of::<OverlayVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_brush = load_ui_font().map(|font| {
            let brush = BrushBuilder::using_font(font)
                .initial_cache_size((4096, 4096))
                .build(device, width, height, format);
            brush.resize_view(width as f32, height as f32, queue);
            brush
        });

        Self {
            rect_pipeline,
            rect_vertex_buffer,
            rect_vertex_capacity: 0,
            rect_vertex_count: 0,
            text_brush,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, queue: &wgpu::Queue) {
        if let Some(brush) = &self.text_brush {
            brush.resize_view(width as f32, height as f32, queue);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_pause_menu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        menu: &PauseMenu,
        runtime: OverlayRuntimeInfo,
        width: u32,
        height: u32,
    ) {
        let width = width.max(1) as f32;
        let height = height.max(1) as f32;
        let layout = PauseMenuLayout::new_with_scenes(width, height, menu.scene_names.len());
        let mut vertices = Vec::with_capacity(420);

        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
            [0.008, 0.012, 0.018, 0.76],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: layout.shell.x + 10.0 * layout.scale,
                y: layout.shell.y + 12.0 * layout.scale,
                ..layout.shell
            },
            [0.0, 0.0, 0.0, 0.32],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            layout.shell,
            [0.035, 0.044, 0.055, 0.985],
        );
        add_outline(
            &mut vertices,
            width,
            height,
            layout.shell,
            [0.24, 0.34, 0.42, 0.95],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            layout.header,
            [0.047, 0.061, 0.076, 0.98],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: layout.header.x,
                y: layout.header.bottom() - (3.0 * layout.scale).max(2.0),
                w: layout.header.w,
                h: (3.0 * layout.scale).max(2.0),
            },
            [0.16, 0.72, 0.78, 0.92],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            layout.footer,
            [0.026, 0.034, 0.043, 0.98],
        );
        add_panel(&mut vertices, width, height, layout.panel);
        add_panel(&mut vertices, width, height, layout.key_panel);

        for button in &layout.resolution_buttons {
            let selected = button.index == menu.selected_resolution;
            add_button(
                &mut vertices,
                width,
                height,
                button.rect,
                selected,
                [0.94, 0.58, 0.14, 1.0],
            );
        }
        add_button(
            &mut vertices,
            width,
            height,
            layout.language_button,
            true,
            [0.16, 0.71, 0.77, 1.0],
        );
        add_button(
            &mut vertices,
            width,
            height,
            layout.god_mode_button,
            menu.god_mode_enabled,
            if menu.god_mode_enabled {
                [0.20, 0.76, 0.48, 1.0]
            } else {
                [0.92, 0.34, 0.30, 1.0]
            },
        );
        add_button(
            &mut vertices,
            width,
            height,
            layout.sensitivity_button,
            true,
            [0.56, 0.48, 0.94, 1.0],
        );
        add_button(
            &mut vertices,
            width,
            height,
            layout.weather_button,
            true,
            [0.30, 0.71, 0.96, 1.0],
        );
        for button in &layout.scene_buttons {
            let current = Some(button.index) == menu.current_scene;
            add_button(
                &mut vertices,
                width,
                height,
                button.rect,
                current,
                [0.94, 0.58, 0.14, 1.0],
            );
        }
        add_button(
            &mut vertices,
            width,
            height,
            layout.resume_button,
            true,
            [0.20, 0.76, 0.48, 1.0],
        );
        add_button(
            &mut vertices,
            width,
            height,
            layout.reset_button,
            false,
            [0.94, 0.58, 0.14, 1.0],
        );
        add_button(
            &mut vertices,
            width,
            height,
            layout.quit_button,
            false,
            [0.92, 0.34, 0.30, 1.0],
        );
        add_card(
            &mut vertices,
            width,
            height,
            layout.status_card,
            [0.94, 0.58, 0.14, 0.9],
        );
        add_card(
            &mut vertices,
            width,
            height,
            layout.movement_card,
            [0.16, 0.71, 0.77, 0.9],
        );
        add_card(
            &mut vertices,
            width,
            height,
            layout.interaction_card,
            [0.94, 0.58, 0.14, 0.9],
        );
        add_card(
            &mut vertices,
            width,
            height,
            layout.simulation_card,
            [0.56, 0.48, 0.94, 0.9],
        );

        self.render_rectangles(
            device,
            queue,
            encoder,
            target,
            &vertices,
            "Pause Menu Rects",
        );
        let sections = build_pause_text_sections(menu, &layout, runtime);
        self.render_text(device, queue, encoder, target, &sections, "Pause Menu Text");
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_hud(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        state: &HudState<'_>,
        width: u32,
        height: u32,
    ) {
        let width = width.max(1) as f32;
        let height = height.max(1) as f32;
        let layout = HudLayout::new(width, height);
        let mut vertices = Vec::with_capacity(420);

        if state.show_help {
            build_help_rectangles(&mut vertices, width, height, &layout);
            self.render_rectangles(device, queue, encoder, target, &vertices, "HUD Help Rects");
            let sections = build_help_text_sections(&layout);
            self.render_text(device, queue, encoder, target, &sections, "HUD Help Text");
            return;
        }

        add_hud_panel(
            &mut vertices,
            width,
            height,
            layout.mode_panel,
            if state.runtime.mode == HudMode::God {
                [0.30, 0.71, 0.96, 0.95]
            } else {
                [0.94, 0.58, 0.14, 0.95]
            },
        );
        add_hud_panel(
            &mut vertices,
            width,
            height,
            layout.telemetry_panel,
            [0.16, 0.71, 0.77, 0.9],
        );
        add_hud_panel(
            &mut vertices,
            width,
            height,
            layout.debug_panel,
            [0.56, 0.48, 0.94, 0.85],
        );
        add_hud_panel(
            &mut vertices,
            width,
            height,
            layout.hint_panel,
            [0.16, 0.71, 0.77, 0.78],
        );
        add_hud_panel(
            &mut vertices,
            width,
            height,
            layout.weapon_panel,
            [0.94, 0.58, 0.14, 0.95],
        );

        let debug_rects = debug_badge_rects(&layout);
        for (rect, enabled, color) in [
            (
                debug_rects[0],
                state.runtime.debug_colliders,
                [0.18, 0.78, 0.84, 1.0],
            ),
            (
                debug_rects[1],
                state.runtime.debug_velocities,
                [0.94, 0.62, 0.18, 1.0],
            ),
            (
                debug_rects[2],
                state.runtime.debug_contacts,
                [0.72, 0.52, 0.98, 1.0],
            ),
            (
                debug_rects[3],
                state.runtime.debug_impulses,
                [1.0, 0.45, 0.12, 1.0],
            ),
        ] {
            add_rect(
                &mut vertices,
                width,
                height,
                rect,
                if enabled {
                    [color[0] * 0.32, color[1] * 0.32, color[2] * 0.32, 0.94]
                } else {
                    [0.07, 0.085, 0.10, 0.88]
                },
            );
            add_outline(
                &mut vertices,
                width,
                height,
                rect,
                if enabled {
                    color
                } else {
                    [0.22, 0.27, 0.31, 0.75]
                },
            );
        }

        let ammo_ratio = if state.max_ammo == 0 {
            0.0
        } else {
            state.current_ammo as f32 / state.max_ammo as f32
        }
        .clamp(0.0, 1.0);
        let reload_ratio = reload_progress(state.reload_timer, state.reload_duration);
        let bar_outer = UiRect {
            x: layout.weapon_panel.x + 18.0 * layout.scale,
            y: layout.weapon_panel.bottom() - 20.0 * layout.scale.max(0.75),
            w: layout.weapon_panel.w - 36.0 * layout.scale,
            h: (6.0 * layout.scale).clamp(4.0, 8.0),
        };
        add_rect(
            &mut vertices,
            width,
            height,
            bar_outer,
            [0.08, 0.095, 0.11, 0.96],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                w: bar_outer.w
                    * if state.is_reloading {
                        reload_ratio
                    } else {
                        ammo_ratio
                    },
                ..bar_outer
            },
            if state.is_reloading {
                [0.18, 0.76, 0.83, 0.98]
            } else if ammo_ratio <= 0.2 {
                [0.96, 0.28, 0.22, 0.98]
            } else {
                [0.96, 0.61, 0.16, 0.98]
            },
        );

        if state.runtime.paused {
            add_rect(
                &mut vertices,
                width,
                height,
                layout.pause_badge,
                [0.48, 0.10, 0.10, 0.93],
            );
            add_outline(
                &mut vertices,
                width,
                height,
                layout.pause_badge,
                [1.0, 0.38, 0.30, 0.96],
            );
        }

        if !state.focus.prompt.is_empty() {
            add_hud_panel(
                &mut vertices,
                width,
                height,
                layout.interaction_panel,
                [0.98, 0.66, 0.20, 1.0],
            );
            let key_size = (42.0 * layout.scale).clamp(31.0, 49.0);
            let key_rect = UiRect {
                x: layout.interaction_panel.x + 14.0 * layout.scale,
                y: layout.interaction_panel.y + (layout.interaction_panel.h - key_size) * 0.5,
                w: key_size,
                h: key_size,
            };
            add_rect(
                &mut vertices,
                width,
                height,
                key_rect,
                [0.94, 0.58, 0.14, 0.98],
            );
            add_outline(
                &mut vertices,
                width,
                height,
                key_rect,
                [1.0, 0.86, 0.50, 1.0],
            );
        }

        // Health bar: bottom-left above the hint panel.
        let health_ratio = if state.max_health > 0.0 {
            (state.health / state.max_health).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let health_outer = UiRect {
            x: layout.hint_panel.x,
            y: layout.hint_panel.y - (26.0 * layout.scale).clamp(18.0, 32.0),
            w: layout.hint_panel.w,
            h: (14.0 * layout.scale).clamp(10.0, 18.0),
        };
        add_rect(
            &mut vertices,
            width,
            height,
            health_outer,
            [0.05, 0.06, 0.08, 0.9],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                w: health_outer.w * health_ratio,
                ..health_outer
            },
            if health_ratio <= 0.25 {
                [0.94, 0.20, 0.16, 0.97]
            } else if health_ratio <= 0.55 {
                [0.95, 0.62, 0.16, 0.97]
            } else {
                [0.28, 0.82, 0.42, 0.97]
            },
        );
        add_outline(
            &mut vertices,
            width,
            height,
            health_outer,
            [0.30, 0.36, 0.42, 0.8],
        );

        // Hurt flash: red vignette borders whose alpha follows the flash value.
        if state.hurt_flash > 0.01 {
            let alpha = (state.hurt_flash * 0.55).clamp(0.0, 0.55);
            let border = (height * 0.10).max(40.0);
            for rect in [
                UiRect {
                    x: 0.0,
                    y: 0.0,
                    w: width,
                    h: border,
                },
                UiRect {
                    x: 0.0,
                    y: height - border,
                    w: width,
                    h: border,
                },
                UiRect {
                    x: 0.0,
                    y: 0.0,
                    w: border,
                    h: height,
                },
                UiRect {
                    x: width - border,
                    y: 0.0,
                    w: border,
                    h: height,
                },
            ] {
                add_rect(
                    &mut vertices,
                    width,
                    height,
                    rect,
                    [0.85, 0.05, 0.05, alpha],
                );
            }
        }

        // Buy menu: centered dark panel.
        if let Some(lines) = state.buy_lines {
            let row_h = (34.0 * layout.scale).clamp(26.0, 40.0);
            let panel_w = (720.0 * layout.scale).clamp(520.0, 860.0).min(width * 0.92);
            let panel_h = row_h * (lines.len() as f32 + 2.5);
            let panel = UiRect {
                x: (width - panel_w) * 0.5,
                y: (height - panel_h) * 0.42,
                w: panel_w,
                h: panel_h,
            };
            add_rect(
                &mut vertices,
                width,
                height,
                panel,
                [0.03, 0.045, 0.06, 0.94],
            );
            add_outline(
                &mut vertices,
                width,
                height,
                panel,
                [0.94, 0.58, 0.14, 0.95],
            );
        }

        // Scoreboard: centered panel, cyan accent.
        if let Some(lines) = state.scoreboard {
            let row_h = (32.0 * layout.scale).clamp(24.0, 38.0);
            let panel_w = (640.0 * layout.scale).clamp(480.0, 780.0).min(width * 0.9);
            let panel_h = row_h * (lines.len() as f32 + 2.2);
            let panel = UiRect {
                x: (width - panel_w) * 0.5,
                y: (height - panel_h) * 0.35,
                w: panel_w,
                h: panel_h,
            };
            add_rect(
                &mut vertices,
                width,
                height,
                panel,
                [0.03, 0.045, 0.06, 0.94],
            );
            add_outline(
                &mut vertices,
                width,
                height,
                panel,
                [0.16, 0.72, 0.78, 0.95],
            );
        }

        // Toast: centered banner near the top.
        if state.toast.is_some() {
            let toast_w = (620.0 * layout.scale).clamp(400.0, 760.0).min(width * 0.9);
            let toast_h = (54.0 * layout.scale).clamp(40.0, 66.0);
            let toast_rect = UiRect {
                x: (width - toast_w) * 0.5,
                y: height * 0.14,
                w: toast_w,
                h: toast_h,
            };
            add_rect(
                &mut vertices,
                width,
                height,
                toast_rect,
                [0.05, 0.07, 0.09, 0.92],
            );
            add_outline(
                &mut vertices,
                width,
                height,
                toast_rect,
                [0.94, 0.58, 0.14, 0.95],
            );
        }

        add_crosshair(
            &mut vertices,
            width,
            height,
            state.aiming,
            state.focus.entity.is_some(),
            layout.scale,
        );
        if state.hit_marker_seconds > 0.0 {
            add_hit_marker(
                &mut vertices,
                width,
                height,
                layout.scale,
                state.hit_marker_seconds,
            );
        }

        self.render_rectangles(device, queue, encoder, target, &vertices, "HUD Rects");
        let sections = build_hud_text_sections(state, &layout, width, height);
        self.render_text(device, queue, encoder, target, &sections, "HUD Text");
    }

    fn render_rectangles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        vertices: &[OverlayVertex],
        label: &str,
    ) {
        self.update_rects(device, queue, vertices);
        if self.rect_vertex_count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.rect_pipeline);
        pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
        pass.draw(0..self.rect_vertex_count, 0..1);
    }

    fn render_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        sections: &[OwnedSection],
        label: &str,
    ) {
        let Some(brush) = &mut self.text_brush else {
            return;
        };
        if let Err(err) = brush.queue(device, queue, sections.iter()) {
            log::warn!("Failed to queue overlay text: {}", err);
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        brush.draw(&mut pass);
    }

    fn update_rects(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[OverlayVertex],
    ) {
        self.rect_vertex_count = vertices.len() as u32;
        if vertices.is_empty() {
            return;
        }
        let data = bytemuck::cast_slice(vertices);
        if vertices.len() > self.rect_vertex_capacity {
            self.rect_vertex_capacity = rect_vertex_capacity(vertices.len());
            self.rect_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Overlay Rect Vertex Buffer"),
                size: rect_vertex_buffer_size(self.rect_vertex_capacity),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        debug_assert!(data.len() as u64 <= self.rect_vertex_buffer.size());
        queue.write_buffer(&self.rect_vertex_buffer, 0, data);
    }
}

fn centered_text_y(rect: UiRect, font_size: f32) -> f32 {
    rect.y + (rect.h - font_size) * 0.43
}

fn scaled_font(base: f32, scale: f32, min: f32, max: f32) -> f32 {
    (base * scale).clamp(min, max)
}

fn section_bounded(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    text: impl Into<String>,
    scale: f32,
    color: [f32; 4],
) -> OwnedSection {
    OwnedSection::default()
        .with_screen_position((x, y))
        .with_bounds((width.max(1.0), height.max(1.0)))
        .add_text(OwnedText::new(text).with_scale(scale).with_color(color))
}

const OVERLAY_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;
