use crate::game::UiRect;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct OverlayVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub(super) fn rect_vertex_capacity(required: usize) -> usize {
    required
        .max(1)
        .checked_next_power_of_two()
        .unwrap_or(required)
}

pub(super) fn rect_vertex_buffer_size(capacity: usize) -> u64 {
    (capacity * std::mem::size_of::<OverlayVertex>()) as u64
}

impl OverlayVertex {
    pub(super) fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
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

pub(super) fn add_panel(vertices: &mut Vec<OverlayVertex>, width: f32, height: f32, rect: UiRect) {
    add_rect(vertices, width, height, rect, [0.050, 0.061, 0.074, 0.97]);
    add_outline(vertices, width, height, rect, [0.19, 0.27, 0.33, 0.88]);
}

pub(super) fn add_button(
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

pub(super) fn add_card(
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

pub(super) fn add_hud_panel(
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

pub(super) fn add_outline(
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

pub(super) fn add_rect(
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

pub(super) fn add_crosshair(
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

pub(super) fn add_hit_marker(
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

pub(super) fn reload_progress(timer: f32, duration: f32) -> f32 {
    if !timer.is_finite() || !duration.is_finite() || duration <= f32::EPSILON {
        return 0.0;
    }
    (1.0 - timer / duration).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
