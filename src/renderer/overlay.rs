use crate::game::{InteractionFocus, PauseMenu, PauseMenuLayout, UiRect, RESOLUTION_OPTIONS};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use wgpu_text::{
    glyph_brush::{ab_glyph::FontArc, OwnedSection, OwnedText},
    BrushBuilder, TextBrush,
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
    pub runtime: OverlayRuntimeInfo,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct OverlayVertex {
    position: [f32; 2],
    color: [f32; 4],
}

fn rect_vertex_capacity(required: usize) -> usize {
    required
        .max(1)
        .checked_next_power_of_two()
        .unwrap_or(required)
}

fn rect_vertex_buffer_size(capacity: usize) -> u64 {
    (capacity * std::mem::size_of::<OverlayVertex>()) as u64
}

impl OverlayVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HudLayout {
    scale: f32,
    mode_panel: UiRect,
    telemetry_panel: UiRect,
    debug_panel: UiRect,
    hint_panel: UiRect,
    weapon_panel: UiRect,
    interaction_panel: UiRect,
    pause_badge: UiRect,
    help_shell: UiRect,
    help_header: UiRect,
    help_cards: [UiRect; 3],
    help_footer: UiRect,
}

impl HudLayout {
    fn new(width: f32, height: f32) -> Self {
        let width = width.max(1.0);
        let height = height.max(1.0);
        let scale = (height / 1080.0).min(width / 1600.0).clamp(0.68, 1.18);
        let margin = (28.0 * scale)
            .clamp(18.0, 36.0)
            .min(width * 0.04)
            .min(height * 0.04);
        let mode_w = (300.0 * scale).clamp(230.0, 350.0);
        let top_h = (82.0 * scale).clamp(58.0, 96.0);
        let telemetry_w = (360.0 * scale).clamp(275.0, 420.0);
        let mode_panel = UiRect {
            x: margin,
            y: margin,
            w: mode_w,
            h: top_h,
        };
        let telemetry_panel = UiRect {
            x: width - margin - telemetry_w,
            y: margin,
            w: telemetry_w,
            h: top_h,
        };
        let debug_panel = UiRect {
            x: margin,
            y: mode_panel.bottom() + (10.0 * scale).max(7.0),
            w: (438.0 * scale).clamp(330.0, 510.0),
            h: (40.0 * scale).clamp(30.0, 47.0),
        };
        let weapon_w = (350.0 * scale).clamp(270.0, 410.0);
        let weapon_h = (118.0 * scale).clamp(84.0, 138.0);
        let weapon_panel = UiRect {
            x: width - margin - weapon_w,
            y: height - margin - weapon_h,
            w: weapon_w,
            h: weapon_h,
        };
        let hint_w = (420.0 * scale).clamp(315.0, 480.0);
        let hint_h = (70.0 * scale).clamp(50.0, 82.0);
        let hint_panel = UiRect {
            x: margin,
            y: height - margin - hint_h,
            w: hint_w,
            h: hint_h,
        };
        let interaction_w = (590.0 * scale)
            .clamp(370.0, 690.0)
            .min(width - margin * 2.0);
        let interaction_h = (82.0 * scale).clamp(58.0, 96.0);
        let interaction_panel = UiRect {
            x: (width - interaction_w) * 0.5,
            y: (height * 0.5 + 56.0 * scale).min(height - margin - interaction_h - weapon_h * 0.18),
            w: interaction_w,
            h: interaction_h,
        };
        let pause_w = (210.0 * scale).clamp(160.0, 245.0);
        let pause_badge = UiRect {
            x: (width - pause_w) * 0.5,
            y: margin,
            w: pause_w,
            h: (40.0 * scale).clamp(30.0, 47.0),
        };

        let help_margin = (40.0 * scale)
            .clamp(20.0, 48.0)
            .min(width * 0.05)
            .min(height * 0.05);
        let help_w = (1180.0 * scale).min(width - help_margin * 2.0).max(1.0);
        let help_h = (720.0 * scale).min(height - help_margin * 2.0).max(1.0);
        let help_shell = UiRect {
            x: (width - help_w) * 0.5,
            y: (height - help_h) * 0.5,
            w: help_w,
            h: help_h,
        };
        let help_header_h = (82.0 * scale).clamp(56.0, 96.0);
        let help_footer_h = (48.0 * scale).clamp(34.0, 56.0);
        let help_header = UiRect {
            x: help_shell.x,
            y: help_shell.y,
            w: help_shell.w,
            h: help_header_h,
        };
        let help_footer = UiRect {
            x: help_shell.x,
            y: help_shell.bottom() - help_footer_h,
            w: help_shell.w,
            h: help_footer_h,
        };
        let card_gap = (14.0 * scale).clamp(9.0, 17.0);
        let card_pad = (20.0 * scale).clamp(13.0, 24.0);
        let cards_y = help_header.bottom() + card_pad;
        let cards_h = (help_footer.y - card_pad - cards_y).max(1.0);
        let cards_w = ((help_shell.w - card_pad * 2.0 - card_gap * 2.0) / 3.0).max(1.0);
        let help_cards = std::array::from_fn(|index| UiRect {
            x: help_shell.x + card_pad + index as f32 * (cards_w + card_gap),
            y: cards_y,
            w: cards_w,
            h: cards_h,
        });

        Self {
            scale,
            mode_panel,
            telemetry_panel,
            debug_panel,
            hint_panel,
            weapon_panel,
            interaction_panel,
            pause_badge,
            help_shell,
            help_header,
            help_cards,
            help_footer,
        }
    }
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
        let layout = PauseMenuLayout::new(width, height);
        let mut vertices = Vec::with_capacity(360);

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
        let sections = build_hud_text_sections(state, &layout);
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

fn load_ui_font() -> Option<FontArc> {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\Noto Sans SC (TrueType).otf",
        "C:\\Windows\\Fonts\\NotoSansSC-VF.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        match FontArc::try_from_vec(bytes) {
            Ok(font) => return Some(font),
            Err(err) => log::warn!("Failed to load UI font {}: {}", path, err),
        }
    }
    log::warn!("No CJK UI font found; overlay text disabled");
    None
}

fn add_panel(vertices: &mut Vec<OverlayVertex>, width: f32, height: f32, rect: UiRect) {
    add_rect(vertices, width, height, rect, [0.050, 0.061, 0.074, 0.97]);
    add_outline(vertices, width, height, rect, [0.19, 0.27, 0.33, 0.88]);
}

fn add_button(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    selected: bool,
    accent: [f32; 4],
) {
    add_rect(
        vertices,
        width,
        height,
        rect,
        if selected {
            [accent[0] * 0.34, accent[1] * 0.34, accent[2] * 0.34, 0.98]
        } else {
            [0.075, 0.091, 0.106, 0.96]
        },
    );
    add_outline(
        vertices,
        width,
        height,
        rect,
        if selected {
            accent
        } else {
            [0.23, 0.30, 0.35, 0.82]
        },
    );
    if selected {
        add_rect(
            vertices,
            width,
            height,
            UiRect {
                x: rect.x,
                y: rect.y,
                w: 4.0_f32.min(rect.w),
                h: rect.h,
            },
            accent,
        );
    }
}

fn add_card(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    accent: [f32; 4],
) {
    add_rect(vertices, width, height, rect, [0.035, 0.045, 0.056, 0.90]);
    add_outline(vertices, width, height, rect, [0.15, 0.21, 0.26, 0.82]);
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x,
            y: rect.y,
            w: 3.0_f32.min(rect.w),
            h: rect.h,
        },
        accent,
    );
}

fn add_hud_panel(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    accent: [f32; 4],
) {
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x + 4.0,
            y: rect.y + 5.0,
            ..rect
        },
        [0.0, 0.0, 0.0, 0.20],
    );
    add_rect(vertices, width, height, rect, [0.018, 0.025, 0.034, 0.82]);
    add_outline(vertices, width, height, rect, [0.16, 0.22, 0.27, 0.76]);
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x,
            y: rect.y,
            w: 3.0_f32.min(rect.w),
            h: rect.h,
        },
        accent,
    );
}

fn add_outline(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    color: [f32; 4],
) {
    let t = 1.5_f32.min(rect.w * 0.5).min(rect.h * 0.5);
    for edge in [
        UiRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: t,
        },
        UiRect {
            x: rect.x,
            y: rect.bottom() - t,
            w: rect.w,
            h: t,
        },
        UiRect {
            x: rect.x,
            y: rect.y,
            w: t,
            h: rect.h,
        },
        UiRect {
            x: rect.right() - t,
            y: rect.y,
            w: t,
            h: rect.h,
        },
    ] {
        add_rect(vertices, width, height, edge, color);
    }
}

fn add_rect(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    color: [f32; 4],
) {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    add_quad(
        vertices,
        width,
        height,
        [
            Vec2::new(rect.x, rect.y),
            Vec2::new(rect.right(), rect.y),
            Vec2::new(rect.right(), rect.bottom()),
            Vec2::new(rect.x, rect.bottom()),
        ],
        color,
    );
}

fn add_line(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    start: Vec2,
    end: Vec2,
    thickness: f32,
    color: [f32; 4],
) {
    let delta = end - start;
    if !delta.is_finite() || delta.length_squared() <= f32::EPSILON {
        return;
    }
    let normal = Vec2::new(-delta.y, delta.x).normalize() * thickness.max(0.5) * 0.5;
    add_quad(
        vertices,
        width,
        height,
        [start + normal, end + normal, end - normal, start - normal],
        color,
    );
}

fn add_quad(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    points: [Vec2; 4],
    color: [f32; 4],
) {
    let to_vertex = |point: Vec2| OverlayVertex {
        position: [to_ndc_x(point.x, width), to_ndc_y(point.y, height)],
        color,
    };
    let quad = points.map(to_vertex);
    vertices.extend_from_slice(&[quad[0], quad[1], quad[2], quad[2], quad[3], quad[0]]);
}

fn to_ndc_x(x: f32, width: f32) -> f32 {
    x / width.max(1.0) * 2.0 - 1.0
}

fn to_ndc_y(y: f32, height: f32) -> f32 {
    1.0 - y / height.max(1.0) * 2.0
}

fn add_crosshair(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    aiming: bool,
    interactive: bool,
    scale: f32,
) {
    let center = Vec2::new(width * 0.5, height * 0.5);
    let gap = if aiming { 2.5 } else { 7.0 } * scale.max(0.78);
    let length = if aiming { 5.0 } else { 9.0 } * scale.max(0.78);
    let thickness = (2.0 * scale).clamp(1.5, 2.5);
    let color = if interactive {
        [1.0, 0.72, 0.22, 0.98]
    } else {
        [0.90, 0.95, 0.98, 0.90]
    };
    add_line(
        vertices,
        width,
        height,
        center - Vec2::X * (gap + length),
        center - Vec2::X * gap,
        thickness,
        color,
    );
    add_line(
        vertices,
        width,
        height,
        center + Vec2::X * gap,
        center + Vec2::X * (gap + length),
        thickness,
        color,
    );
    add_line(
        vertices,
        width,
        height,
        center - Vec2::Y * (gap + length),
        center - Vec2::Y * gap,
        thickness,
        color,
    );
    add_line(
        vertices,
        width,
        height,
        center + Vec2::Y * gap,
        center + Vec2::Y * (gap + length),
        thickness,
        color,
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: center.x - 1.5,
            y: center.y - 1.5,
            w: 3.0,
            h: 3.0,
        },
        color,
    );
}

fn add_hit_marker(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    scale: f32,
    seconds: f32,
) {
    let center = Vec2::new(width * 0.5, height * 0.5);
    let gap = (12.0 * scale).clamp(9.0, 15.0);
    let length = (8.0 * scale).clamp(6.0, 10.0);
    let alpha = (seconds * 9.0).clamp(0.45, 1.0);
    let color = [1.0, 0.94, 0.78, alpha];
    for direction in [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ] {
        let direction = direction.normalize();
        add_line(
            vertices,
            width,
            height,
            center + direction * gap,
            center + direction * (gap + length),
            (2.3 * scale).clamp(1.8, 3.0),
            color,
        );
    }
}

fn debug_badge_rects(layout: &HudLayout) -> [UiRect; 3] {
    let pad = (6.0 * layout.scale).clamp(4.0, 7.0);
    let gap = (6.0 * layout.scale).clamp(4.0, 7.0);
    let badge_w = (layout.debug_panel.w - pad * 2.0 - gap * 2.0) / 3.0;
    std::array::from_fn(|index| UiRect {
        x: layout.debug_panel.x + pad + index as f32 * (badge_w + gap),
        y: layout.debug_panel.y + pad,
        w: badge_w,
        h: layout.debug_panel.h - pad * 2.0,
    })
}

fn reload_progress(timer: f32, duration: f32) -> f32 {
    if !timer.is_finite() || !duration.is_finite() || duration <= f32::EPSILON {
        return 0.0;
    }
    (1.0 - timer / duration).clamp(0.0, 1.0)
}

fn build_help_rectangles(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    layout: &HudLayout,
) {
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        },
        [0.005, 0.009, 0.014, 0.72],
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: layout.help_shell.x + 8.0 * layout.scale,
            y: layout.help_shell.y + 10.0 * layout.scale,
            ..layout.help_shell
        },
        [0.0, 0.0, 0.0, 0.30],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_shell,
        [0.030, 0.040, 0.051, 0.98],
    );
    add_outline(
        vertices,
        width,
        height,
        layout.help_shell,
        [0.22, 0.33, 0.41, 0.94],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_header,
        [0.048, 0.063, 0.078, 0.98],
    );
    add_rect(
        vertices,
        width,
        height,
        layout.help_footer,
        [0.024, 0.032, 0.041, 0.98],
    );
    for (card, color) in layout.help_cards.iter().zip([
        [0.16, 0.71, 0.77, 0.95],
        [0.94, 0.58, 0.14, 0.95],
        [0.56, 0.48, 0.94, 0.95],
    ]) {
        add_card(vertices, width, height, *card, color);
    }
}

fn build_pause_text_sections(
    menu: &PauseMenu,
    layout: &PauseMenuLayout,
    runtime: OverlayRuntimeInfo,
) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(24);
    let text = [0.91, 0.94, 0.96, 1.0];
    let muted = [0.58, 0.66, 0.71, 1.0];
    let cyan = [0.30, 0.82, 0.86, 1.0];
    let orange = [1.0, 0.68, 0.28, 1.0];
    let violet = [0.72, 0.64, 1.0, 1.0];
    let s = layout.scale;
    let header_pad = (28.0 * s).clamp(19.0, 34.0);

    sections.push(section_bounded(
        layout.header.x + header_pad,
        layout.header.y + (13.0 * s).clamp(9.0, 16.0),
        layout.header.w * 0.65,
        layout.header.h,
        "GXENGINE  /  PHYSICS LAB",
        scaled_font(27.0, s, 20.0, 33.0),
        text,
    ));
    sections.push(section_bounded(
        layout.header.x + header_pad,
        layout.header.y + layout.header.h * 0.56,
        layout.header.w * 0.65,
        layout.header.h * 0.4,
        "暂停菜单 · 实时实验台设置",
        scaled_font(15.0, s, 12.0, 18.0),
        muted,
    ));
    sections.push(section_bounded(
        layout.header.right() - (170.0 * s).clamp(122.0, 205.0),
        layout.header.y + layout.header.h * 0.34,
        (150.0 * s).clamp(110.0, 180.0),
        layout.header.h * 0.5,
        "ESC  返回实验",
        scaled_font(17.0, s, 13.0, 20.0),
        cyan,
    ));

    let panel_pad = (24.0 * s).clamp(16.0, 30.0);
    sections.push(section_bounded(
        layout.panel.x + panel_pad,
        layout.panel.y + (17.0 * s).clamp(11.0, 21.0),
        layout.panel.w - panel_pad * 2.0,
        40.0 * s,
        "显示与体验",
        scaled_font(22.0, s, 17.0, 27.0),
        text,
    ));
    sections.push(section_bounded(
        layout.panel.x + panel_pad,
        layout.resolution_buttons[0].rect.y - (24.0 * s).clamp(17.0, 29.0),
        layout.panel.w - panel_pad * 2.0,
        24.0 * s,
        "窗口分辨率",
        scaled_font(14.0, s, 11.0, 17.0),
        orange,
    ));
    for button in &layout.resolution_buttons {
        let font = scaled_font(17.0, s, 13.0, 20.0);
        let selected = button.index == menu.selected_resolution;
        sections.push(section_bounded(
            button.rect.x + (14.0 * s).clamp(10.0, 17.0),
            centered_text_y(button.rect, font),
            button.rect.w - 24.0 * s,
            button.rect.h,
            if selected {
                format!("●  {}", RESOLUTION_OPTIONS[button.index].label)
            } else {
                RESOLUTION_OPTIONS[button.index].label.to_string()
            },
            font,
            if selected {
                [1.0, 0.90, 0.70, 1.0]
            } else {
                text
            },
        ));
    }

    sections.push(section_bounded(
        layout.language_button.x,
        layout.language_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.language_button.w,
        24.0 * s,
        "界面语言",
        scaled_font(14.0, s, 11.0, 17.0),
        cyan,
    ));
    let button_font = scaled_font(17.0, s, 13.0, 20.0);
    sections.push(section_bounded(
        layout.language_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.language_button, button_font),
        layout.language_button.w - 24.0 * s,
        layout.language_button.h,
        "简体中文  ·  ZH-CN",
        button_font,
        text,
    ));
    sections.push(section_bounded(
        layout.god_mode_button.x,
        layout.god_mode_button.y - (24.0 * s).clamp(17.0, 29.0),
        layout.god_mode_button.w,
        24.0 * s,
        "观察权限",
        scaled_font(14.0, s, 11.0, 17.0),
        if menu.god_mode_enabled { cyan } else { orange },
    ));
    sections.push(section_bounded(
        layout.god_mode_button.x + (14.0 * s).clamp(10.0, 17.0),
        centered_text_y(layout.god_mode_button, button_font),
        layout.god_mode_button.w - 24.0 * s,
        layout.god_mode_button.h,
        if menu.god_mode_enabled {
            "上帝观察模式  ·  已启用"
        } else {
            "上帝观察模式  ·  已禁用"
        },
        button_font,
        text,
    ));

    let status_pad = (15.0 * s).clamp(10.0, 18.0);
    sections.push(section_bounded(
        layout.status_card.x + status_pad,
        layout.status_card.y + status_pad * 0.7,
        layout.status_card.w - status_pad * 2.0,
        layout.status_card.h - status_pad,
        format!(
            "当前会话  {} · {} · {:.0} FPS\n诊断 {}/3     灯光 {}     动态刚体 {}{}",
            runtime.mode.short_label(),
            runtime.time_label(),
            runtime.fps.max(0.0),
            runtime.enabled_debug_count(),
            runtime.light_count,
            runtime.dynamic_body_count,
            if runtime.holding_object {
                format!("     持物 {:.1}m", runtime.hold_distance)
            } else {
                String::new()
            },
        ),
        scaled_font(14.0, s, 11.0, 17.0),
        muted,
    ));

    sections.push(section_bounded(
        layout.key_panel.x + panel_pad,
        layout.key_panel.y + (17.0 * s).clamp(11.0, 21.0),
        layout.key_panel.w - panel_pad * 2.0,
        40.0 * s,
        "操作指南",
        scaled_font(22.0, s, 17.0, 27.0),
        text,
    ));
    add_key_card_text(
        &mut sections,
        layout.movement_card,
        s,
        "移动与视角",
        "WASD  移动       Shift  奔跑 / 加速\n鼠标  环顾       Space  跳跃 / 上升\nCtrl  下降       V  切换 FPS / GOD",
        cyan,
    );
    add_key_card_text(
        &mut sections,
        layout.interaction_card,
        s,
        "交互与沙盒",
        "左键  射击    右键  瞄准    R  装填\nE / 中键  抓取或放下物体\n滚轮  调整距离       T  投掷 / 推动\nX  冻结       Delete  删除\nG  箱子       B  弹力球       H  重箱",
        orange,
    );
    add_key_card_text(
        &mut sections,
        layout.simulation_card,
        s,
        "时间与诊断",
        "P  暂停       .  单步       [ / ]  倍速\nF3  碰撞体       F4  速度       F5  接触点\nF  按住完整帮助       Esc  设置",
        violet,
    );

    sections.push(section_bounded(
        layout.footer.x + header_pad,
        layout.footer.y + layout.footer.h * 0.28,
        layout.footer.w * 0.72,
        layout.footer.h,
        "提示：分辨率切换保持窗口化并自动居中 · 所有实验状态会继续保留",
        scaled_font(13.0, s, 10.0, 15.0),
        muted,
    ));
    sections
}

fn add_key_card_text(
    sections: &mut Vec<OwnedSection>,
    rect: UiRect,
    scale: f32,
    title: &str,
    body: &str,
    accent: [f32; 4],
) {
    let pad = (16.0 * scale).clamp(10.0, 19.0);
    let title_font = scaled_font(17.0, scale, 13.0, 20.0);
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad * 0.72,
        rect.w - pad * 2.0,
        title_font * 1.4,
        title,
        title_font,
        accent,
    ));
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad + title_font * 1.5,
        rect.w - pad * 2.0,
        rect.h - pad * 2.0 - title_font,
        body,
        scaled_font(14.5, scale, 11.0, 17.0),
        [0.84, 0.89, 0.92, 1.0],
    ));
}

fn build_hud_text_sections(state: &HudState<'_>, layout: &HudLayout) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(18);
    let text = [0.91, 0.95, 0.97, 1.0];
    let muted = [0.60, 0.68, 0.73, 1.0];
    let s = layout.scale;
    let pad = (16.0 * s).clamp(11.0, 19.0);

    sections.push(section_bounded(
        layout.mode_panel.x + pad,
        layout.mode_panel.y + (10.0 * s).clamp(7.0, 12.0),
        layout.mode_panel.w - pad * 2.0,
        layout.mode_panel.h * 0.52,
        format!(
            "{}  /  {}",
            state.runtime.mode.short_label(),
            state.runtime.mode.description()
        ),
        scaled_font(19.0, s, 15.0, 23.0),
        text,
    ));
    sections.push(section_bounded(
        layout.mode_panel.x + pad,
        layout.mode_panel.y + layout.mode_panel.h * 0.58,
        layout.mode_panel.w - pad * 2.0,
        layout.mode_panel.h * 0.34,
        state.runtime.time_label(),
        scaled_font(13.0, s, 11.0, 16.0),
        if state.runtime.paused {
            [1.0, 0.48, 0.38, 1.0]
        } else {
            [0.42, 0.82, 0.86, 1.0]
        },
    ));

    sections.push(section_bounded(
        layout.telemetry_panel.x + pad,
        layout.telemetry_panel.y + (9.0 * s).clamp(6.0, 11.0),
        layout.telemetry_panel.w - pad * 2.0,
        layout.telemetry_panel.h * 0.44,
        format!(
            "{:.0} FPS  ·  {} LIGHTS  ·  {} BODIES",
            state.runtime.fps.max(0.0),
            state.runtime.light_count,
            state.runtime.dynamic_body_count
        ),
        scaled_font(17.0, s, 13.0, 21.0),
        [0.44, 0.86, 0.89, 1.0],
    ));
    sections.push(section_bounded(
        layout.telemetry_panel.x + pad,
        layout.telemetry_panel.y + layout.telemetry_panel.h * 0.56,
        layout.telemetry_panel.w - pad * 2.0,
        layout.telemetry_panel.h * 0.35,
        format!(
            "POS  {:+.1}  {:+.1}  {:+.1}",
            state.runtime.camera_position[0],
            state.runtime.camera_position[1],
            state.runtime.camera_position[2]
        ),
        scaled_font(13.0, s, 10.5, 16.0),
        muted,
    ));

    for ((rect, label), enabled) in debug_badge_rects(layout)
        .into_iter()
        .zip(["F3  碰撞", "F4  速度", "F5  接触"])
        .zip([
            state.runtime.debug_colliders,
            state.runtime.debug_velocities,
            state.runtime.debug_contacts,
        ])
    {
        let font = scaled_font(12.5, s, 10.0, 15.0);
        sections.push(section_bounded(
            rect.x + (7.0 * s).clamp(5.0, 9.0),
            centered_text_y(rect, font),
            rect.w - 12.0 * s,
            rect.h,
            label,
            font,
            if enabled { text } else { muted },
        ));
    }

    sections.push(section_bounded(
        layout.hint_panel.x + pad,
        layout.hint_panel.y + (8.0 * s).clamp(6.0, 10.0),
        layout.hint_panel.w - pad * 2.0,
        layout.hint_panel.h,
        if state.runtime.holding_object {
            format!(
                "持物中  {:.1}m     ·     滚轮  调距\nT  投掷     X  冻结     E / 中键  放下",
                state.runtime.hold_distance
            )
        } else {
            "F  操作指南     ·     ESC  设置\nG / B / H  生成     E  抓取     T  推动".to_string()
        },
        scaled_font(13.0, s, 10.5, 16.0),
        [0.74, 0.82, 0.86, 1.0],
    ));

    sections.push(section_bounded(
        layout.weapon_panel.x + pad,
        layout.weapon_panel.y + (9.0 * s).clamp(6.0, 11.0),
        layout.weapon_panel.w * 0.58,
        layout.weapon_panel.h * 0.42,
        state.weapon_name,
        scaled_font(15.0, s, 12.0, 18.0),
        [1.0, 0.70, 0.30, 1.0],
    ));
    sections.push(section_bounded(
        layout.weapon_panel.x + pad,
        layout.weapon_panel.y + layout.weapon_panel.h * 0.40,
        layout.weapon_panel.w - pad * 2.0,
        layout.weapon_panel.h * 0.42,
        if state.is_reloading {
            format!("装填中  {:.1}s", state.reload_timer.max(0.0))
        } else {
            format!("{:02}  /  {:02}", state.current_ammo, state.max_ammo)
        },
        scaled_font(27.0, s, 20.0, 32.0),
        if !state.is_reloading && state.current_ammo * 5 <= state.max_ammo {
            [1.0, 0.40, 0.30, 1.0]
        } else {
            text
        },
    ));

    if state.runtime.paused {
        let font = scaled_font(14.0, s, 11.0, 17.0);
        sections.push(section_bounded(
            layout.pause_badge.x + 12.0 * s,
            centered_text_y(layout.pause_badge, font),
            layout.pause_badge.w - 24.0 * s,
            layout.pause_badge.h,
            "Ⅱ  模拟暂停  ·  . 单步",
            font,
            [1.0, 0.88, 0.84, 1.0],
        ));
    }

    if !state.focus.prompt.is_empty() {
        let key_font = scaled_font(17.0, s, 13.0, 20.0);
        let key_size = (42.0 * s).clamp(31.0, 49.0);
        sections.push(section_bounded(
            layout.interaction_panel.x + 14.0 * s,
            layout.interaction_panel.y
                + (layout.interaction_panel.h - key_size) * 0.5
                + (key_size - key_font) * 0.36,
            key_size,
            key_size,
            "E",
            key_font,
            [0.10, 0.075, 0.035, 1.0],
        ));
        let text_x = layout.interaction_panel.x + key_size + 28.0 * s;
        sections.push(section_bounded(
            text_x,
            layout.interaction_panel.y + (8.0 * s).clamp(6.0, 10.0),
            layout.interaction_panel.right() - text_x - 14.0 * s,
            layout.interaction_panel.h * 0.46,
            format!("{}   ·   {:.1}m", state.focus.title, state.focus.distance),
            scaled_font(17.0, s, 13.0, 20.0),
            [1.0, 0.78, 0.39, 1.0],
        ));
        sections.push(section_bounded(
            text_x,
            layout.interaction_panel.y + layout.interaction_panel.h * 0.55,
            layout.interaction_panel.right() - text_x - 14.0 * s,
            layout.interaction_panel.h * 0.35,
            &state.focus.prompt,
            scaled_font(13.0, s, 10.5, 16.0),
            text,
        ));
    }
    sections
}

fn build_help_text_sections(layout: &HudLayout) -> Vec<OwnedSection> {
    let mut sections = Vec::with_capacity(10);
    let s = layout.scale;
    let text = [0.91, 0.95, 0.97, 1.0];
    let muted = [0.60, 0.68, 0.73, 1.0];
    let header_pad = (28.0 * s).clamp(18.0, 34.0);
    sections.push(section_bounded(
        layout.help_header.x + header_pad,
        layout.help_header.y + (13.0 * s).clamp(9.0, 16.0),
        layout.help_header.w * 0.7,
        layout.help_header.h,
        "操作指南  /  CONTROLS",
        scaled_font(26.0, s, 20.0, 31.0),
        text,
    ));
    sections.push(section_bounded(
        layout.help_header.x + header_pad,
        layout.help_header.y + layout.help_header.h * 0.60,
        layout.help_header.w * 0.7,
        layout.help_header.h * 0.32,
        "按住 F 查看 · 松开立即返回实验",
        scaled_font(13.0, s, 10.5, 16.0),
        muted,
    ));
    add_help_card_text(
        &mut sections,
        layout.help_cards[0],
        s,
        "01  移动 / 观察",
        "WASD\n移动\n\nSHIFT\n奔跑 / 加速飞行\n\nSPACE / CTRL\n跳跃、上升 / 下降\n\nV\nFPS / 上帝模式",
        [0.36, 0.84, 0.88, 1.0],
    );
    add_help_card_text(
        &mut sections,
        layout.help_cards[1],
        s,
        "02  交互 / 沙盒",
        "左键 / 右键 / R\n射击 / 瞄准 / 装填\n\nE / 中键 / 滚轮\n抓取、放下 / 调整距离\n\nT / X / DELETE\n投掷、冻结 / 删除\n\nG / B / H\n箱子 / 弹力球 / 重箱",
        [1.0, 0.70, 0.32, 1.0],
    );
    add_help_card_text(
        &mut sections,
        layout.help_cards[2],
        s,
        "03  模拟 / 诊断",
        "P / . / [ ]\n暂停 / 单步 / 调整倍速\n\nF3\n碰撞体线框\n\nF4 / F5\n速度向量 / 接触点\n\nESC\n打开设置菜单",
        [0.76, 0.68, 1.0, 1.0],
    );
    sections.push(section_bounded(
        layout.help_footer.x + header_pad,
        layout.help_footer.y + layout.help_footer.h * 0.29,
        layout.help_footer.w - header_pad * 2.0,
        layout.help_footer.h,
        "准星变为橙色时，当前物体可交互 · HUD 会持续显示模拟速度与诊断图层",
        scaled_font(13.0, s, 10.0, 15.0),
        muted,
    ));
    sections
}

fn add_help_card_text(
    sections: &mut Vec<OwnedSection>,
    rect: UiRect,
    scale: f32,
    title: &str,
    body: &str,
    accent: [f32; 4],
) {
    let pad = (20.0 * scale).clamp(13.0, 24.0);
    let title_font = scaled_font(18.0, scale, 14.0, 22.0);
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad,
        rect.w - pad * 2.0,
        title_font * 1.5,
        title,
        title_font,
        accent,
    ));
    sections.push(section_bounded(
        rect.x + pad,
        rect.y + pad + title_font * 2.2,
        rect.w - pad * 2.0,
        rect.h - pad * 2.0 - title_font * 2.0,
        body,
        scaled_font(14.5, scale, 11.0, 17.0),
        [0.84, 0.89, 0.92, 1.0],
    ));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_inside(rect: UiRect, width: f32, height: f32) {
        assert!(rect.x >= -0.01, "left edge escaped: {rect:?}");
        assert!(rect.y >= -0.01, "top edge escaped: {rect:?}");
        assert!(rect.right() <= width + 0.01, "right edge escaped: {rect:?}");
        assert!(
            rect.bottom() <= height + 0.01,
            "bottom edge escaped: {rect:?}"
        );
    }

    fn overlaps(a: UiRect, b: UiRect) -> bool {
        a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y
    }

    #[test]
    fn hud_layout_stays_inside_supported_viewports() {
        for (width, height) in [
            (1280.0, 720.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3440.0, 1440.0),
        ] {
            let layout = HudLayout::new(width, height);
            for rect in [
                layout.mode_panel,
                layout.telemetry_panel,
                layout.debug_panel,
                layout.hint_panel,
                layout.weapon_panel,
                layout.interaction_panel,
                layout.pause_badge,
                layout.help_shell,
                layout.help_header,
                layout.help_footer,
                layout.help_cards[0],
                layout.help_cards[1],
                layout.help_cards[2],
            ] {
                assert_inside(rect, width, height);
            }
            assert!(!overlaps(layout.mode_panel, layout.telemetry_panel));
            assert!(!overlaps(layout.hint_panel, layout.weapon_panel));
        }
    }

    #[test]
    fn diagonal_hit_marker_builds_four_finite_quads() {
        let mut vertices = Vec::new();
        add_hit_marker(&mut vertices, 1280.0, 720.0, 0.68, 0.08);
        assert_eq!(vertices.len(), 24);
        assert!(vertices
            .iter()
            .all(|vertex| vertex.position.into_iter().all(f32::is_finite)));
    }

    #[test]
    fn reload_progress_is_bounded_and_rejects_invalid_input() {
        assert_eq!(reload_progress(2.0, 2.0), 0.0);
        assert!((reload_progress(1.0, 2.0) - 0.5).abs() < 1e-6);
        assert_eq!(reload_progress(-1.0, 2.0), 1.0);
        assert_eq!(reload_progress(f32::NAN, 2.0), 0.0);
        assert_eq!(reload_progress(1.0, 0.0), 0.0);
    }

    #[test]
    fn rect_buffer_allocation_matches_the_recorded_growth_capacity() {
        let capacity = rect_vertex_capacity(414);
        assert_eq!(capacity, 512);
        assert!(
            rect_vertex_buffer_size(capacity)
                >= (438 * std::mem::size_of::<OverlayVertex>()) as u64
        );
    }
}
