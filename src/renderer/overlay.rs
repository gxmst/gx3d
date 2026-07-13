use crate::game::{InteractionFocus, PauseMenu, PauseMenuLayout, UiRect, RESOLUTION_OPTIONS};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgpu_text::{
    glyph_brush::{ab_glyph::FontArc, OwnedSection, OwnedText},
    BrushBuilder, TextBrush,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct OverlayVertex {
    position: [f32; 2],
    color: [f32; 4],
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
        width: u32,
        height: u32,
    ) {
        let width = width as f32;
        let height = height as f32;
        let layout = PauseMenuLayout::new(width, height);

        let mut vertices = Vec::new();
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
            [0.02, 0.025, 0.03, 0.62],
        );
        add_panel(&mut vertices, width, height, layout.panel);
        add_panel(&mut vertices, width, height, layout.key_panel);

        for button in &layout.resolution_buttons {
            let selected = button.index == menu.selected_resolution;
            add_rect(
                &mut vertices,
                width,
                height,
                button.rect,
                if selected {
                    [0.96, 0.63, 0.18, 0.92]
                } else {
                    [0.16, 0.19, 0.22, 0.88]
                },
            );
            add_outline(
                &mut vertices,
                width,
                height,
                button.rect,
                if selected {
                    [1.0, 0.84, 0.42, 1.0]
                } else {
                    [0.34, 0.42, 0.48, 0.8]
                },
            );
        }
        add_rect(
            &mut vertices,
            width,
            height,
            layout.language_button,
            [0.12, 0.28, 0.32, 0.9],
        );
        add_outline(
            &mut vertices,
            width,
            height,
            layout.language_button,
            [0.35, 0.78, 0.82, 0.95],
        );
        add_rect(
            &mut vertices,
            width,
            height,
            layout.god_mode_button,
            if menu.god_mode_enabled {
                [0.12, 0.42, 0.28, 0.9]
            } else {
                [0.30, 0.18, 0.18, 0.9]
            },
        );
        add_outline(
            &mut vertices,
            width,
            height,
            layout.god_mode_button,
            [0.42, 0.82, 0.58, 0.95],
        );

        self.update_rects(device, queue, &vertices);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Rect Pass"),
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
            if self.rect_vertex_count > 0 {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, self.rect_vertex_buffer.slice(..));
                pass.draw(0..self.rect_vertex_count, 0..1);
            }
        }

        let sections = build_text_sections(menu, &layout);
        if let Some(brush) = &mut self.text_brush {
            if let Err(err) = brush.queue(device, queue, sections.iter()) {
                log::warn!("Failed to queue overlay text: {}", err);
                return;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Text Pass"),
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
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_hud(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        focus: &InteractionFocus,
        aiming: bool,
        width: u32,
        height: u32,
    ) {
        let (width, height) = (width as f32, height as f32);
        let center_x = width * 0.5;
        let center_y = height * 0.5;
        let gap = if aiming { 2.0 } else { 6.0 };
        let length = if aiming { 5.0 } else { 8.0 };
        let color = if focus.entity.is_some() {
            [1.0, 0.78, 0.28, 0.96]
        } else {
            [0.92, 0.96, 1.0, 0.88]
        };
        let mut vertices = Vec::new();
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: center_x - gap - length,
                y: center_y - 1.0,
                w: length,
                h: 2.0,
            },
            color,
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: center_x + gap,
                y: center_y - 1.0,
                w: length,
                h: 2.0,
            },
            color,
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: center_x - 1.0,
                y: center_y - gap - length,
                w: 2.0,
                h: length,
            },
            color,
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: center_x - 1.0,
                y: center_y + gap,
                w: 2.0,
                h: length,
            },
            color,
        );
        add_rect(
            &mut vertices,
            width,
            height,
            UiRect {
                x: center_x - 1.5,
                y: center_y - 1.5,
                w: 3.0,
                h: 3.0,
            },
            color,
        );

        if !focus.prompt.is_empty() {
            let panel = UiRect {
                x: center_x - 250.0,
                y: center_y + 48.0,
                w: 500.0,
                h: 62.0,
            };
            add_rect(
                &mut vertices,
                width,
                height,
                panel,
                [0.025, 0.03, 0.035, 0.78],
            );
            add_outline(&mut vertices, width, height, panel, [0.95, 0.62, 0.20, 0.8]);
        }
        self.update_rects(device, queue, &vertices);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD Rect Pass"),
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

        if !focus.prompt.is_empty() {
            if let Some(brush) = &mut self.text_brush {
                let sections = [
                    section(
                        center_x - 226.0,
                        center_y + 59.0,
                        &focus.title,
                        19.0,
                        [1.0, 0.82, 0.46, 1.0],
                    ),
                    section(
                        center_x - 226.0,
                        center_y + 84.0,
                        &focus.prompt,
                        16.0,
                        [0.92, 0.95, 0.98, 1.0],
                    ),
                ];
                if brush.queue(device, queue, sections.iter()).is_ok() {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("HUD Text Pass"),
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
            }
        }
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
            self.rect_vertex_capacity = vertices.len().next_power_of_two();
            self.rect_vertex_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Overlay Rect Vertex Buffer"),
                    contents: data,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
        } else {
            queue.write_buffer(&self.rect_vertex_buffer, 0, data);
        }
    }
}

fn load_ui_font() -> Option<FontArc> {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\Noto Sans SC (TrueType).otf",
        "C:\\Windows\\Fonts\\NotoSansSC-VF.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
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
    log::warn!("No Simplified Chinese UI font found; pause menu text disabled");
    None
}

fn add_panel(vertices: &mut Vec<OverlayVertex>, width: f32, height: f32, rect: UiRect) {
    add_rect(vertices, width, height, rect, [0.055, 0.065, 0.075, 0.92]);
    add_outline(vertices, width, height, rect, [0.34, 0.42, 0.48, 0.85]);
}

fn add_outline(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    color: [f32; 4],
) {
    let t = 1.5;
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: t,
        },
        color,
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x,
            y: rect.y + rect.h - t,
            w: rect.w,
            h: t,
        },
        color,
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x,
            y: rect.y,
            w: t,
            h: rect.h,
        },
        color,
    );
    add_rect(
        vertices,
        width,
        height,
        UiRect {
            x: rect.x + rect.w - t,
            y: rect.y,
            w: t,
            h: rect.h,
        },
        color,
    );
}

fn add_rect(
    vertices: &mut Vec<OverlayVertex>,
    width: f32,
    height: f32,
    rect: UiRect,
    color: [f32; 4],
) {
    let x0 = to_ndc_x(rect.x, width);
    let x1 = to_ndc_x(rect.x + rect.w, width);
    let y0 = to_ndc_y(rect.y, height);
    let y1 = to_ndc_y(rect.y + rect.h, height);
    vertices.extend_from_slice(&[
        OverlayVertex {
            position: [x0, y0],
            color,
        },
        OverlayVertex {
            position: [x1, y0],
            color,
        },
        OverlayVertex {
            position: [x1, y1],
            color,
        },
        OverlayVertex {
            position: [x1, y1],
            color,
        },
        OverlayVertex {
            position: [x0, y1],
            color,
        },
        OverlayVertex {
            position: [x0, y0],
            color,
        },
    ]);
}

fn to_ndc_x(x: f32, width: f32) -> f32 {
    x / width * 2.0 - 1.0
}

fn to_ndc_y(y: f32, height: f32) -> f32 {
    1.0 - y / height * 2.0
}

fn build_text_sections(menu: &PauseMenu, layout: &PauseMenuLayout) -> Vec<OwnedSection> {
    let mut sections = Vec::new();
    let text = [0.92, 0.94, 0.94, 1.0];
    let muted = [0.66, 0.72, 0.76, 1.0];
    let accent = [0.35, 0.83, 0.86, 1.0];
    let selected = [0.08, 0.07, 0.045, 1.0];

    sections.push(section(
        layout.panel.x + 42.0,
        layout.panel.y + 44.0,
        "设置",
        34.0,
        text,
    ));
    sections.push(section(
        layout.panel.x + 42.0,
        layout.god_mode_button.y - 34.0,
        "观察模式",
        22.0,
        accent,
    ));
    sections.push(section(
        layout.god_mode_button.x + 22.0,
        layout.god_mode_button.y + 11.0,
        if menu.god_mode_enabled {
            "上帝模式：开启"
        } else {
            "上帝模式：关闭"
        },
        19.0,
        text,
    ));
    sections.push(section(
        layout.panel.x + 42.0,
        layout.panel.y + 96.0,
        "按 ESC 返回游戏",
        20.0,
        muted,
    ));
    sections.push(section(
        layout.panel.x + 42.0,
        layout.panel.y + 132.0,
        "分辨率",
        22.0,
        accent,
    ));

    for button in &layout.resolution_buttons {
        let color = if button.index == menu.selected_resolution {
            selected
        } else {
            text
        };
        sections.push(section(
            button.rect.x + 22.0,
            button.rect.y + 11.0,
            RESOLUTION_OPTIONS[button.index].label,
            19.0,
            color,
        ));
    }

    sections.push(section(
        layout.panel.x + 42.0,
        layout.language_button.y - 36.0,
        "语言",
        22.0,
        accent,
    ));
    sections.push(section(
        layout.language_button.x + 22.0,
        layout.language_button.y + 11.0,
        "简体中文",
        19.0,
        text,
    ));
    sections.push(section(
        layout.panel.x + 42.0,
        layout.panel.y + layout.panel.h - 64.0,
        "更改分辨率后窗口会自动保持窗口化并尽量居中。",
        18.0,
        muted,
    ));

    sections.push(section(
        layout.key_panel.x + 42.0,
        layout.key_panel.y + 44.0,
        "键位提示",
        32.0,
        text,
    ));
    sections.push(section(
        layout.key_panel.x + 42.0,
        layout.key_panel.y + 98.0,
        "WASD  移动\nShift  奔跑 / 加速飞行\nSpace  跳跃 / 上升    Ctrl  下降\nV  FPS / 上帝模式\nP  暂停    .  单步    [ / ]  调整倍速\nF3  碰撞线框    F4  速度向量    F5  接触点\n鼠标左键  射击    鼠标右键  瞄准\nE  开门 / 抓取    鼠标中键  抓取或放下\n鼠标滚轮  调整持物距离\nT  投掷持物 / 推开准星物体\nX  冻结 / 解冻准星物体\nDelete  删除准星物体\nG  生成箱子    B  生成弹力球    H  生成重箱\nF  按住显示键位提示\nEsc  打开或关闭设置",
        21.0,
        text,
    ));

    sections
}

fn section(x: f32, y: f32, text: impl Into<String>, scale: f32, color: [f32; 4]) -> OwnedSection {
    OwnedSection::default()
        .with_screen_position((x, y))
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
