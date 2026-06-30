use super::{Mesh, Vertex};
use glam::{Vec2, Vec3};

pub struct ProceduralGenerator;

impl ProceduralGenerator {
    pub fn create_cube() -> Mesh {
        let vertices = vec![
            // Front face
            Vertex::new(Vec3::new(-0.5, -0.5, 0.5), Vec3::Z, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, 0.5), Vec3::Z, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(0.5, 0.5, 0.5), Vec3::Z, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, 0.5), Vec3::Z, Vec2::new(0.0, 0.0)),
            // Back face
            Vertex::new(Vec3::new(0.5, -0.5, -0.5), -Vec3::Z, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, -0.5), -Vec3::Z, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, -0.5), -Vec3::Z, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(0.5, 0.5, -0.5), -Vec3::Z, Vec2::new(0.0, 0.0)),
            // Top face
            Vertex::new(Vec3::new(-0.5, 0.5, 0.5), Vec3::Y, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, 0.5, 0.5), Vec3::Y, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(0.5, 0.5, -0.5), Vec3::Y, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, -0.5), Vec3::Y, Vec2::new(0.0, 0.0)),
            // Bottom face
            Vertex::new(Vec3::new(-0.5, -0.5, -0.5), -Vec3::Y, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, -0.5), -Vec3::Y, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, 0.5), -Vec3::Y, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, 0.5), -Vec3::Y, Vec2::new(0.0, 0.0)),
            // Right face
            Vertex::new(Vec3::new(0.5, -0.5, 0.5), Vec3::X, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(0.5, -0.5, -0.5), Vec3::X, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(0.5, 0.5, -0.5), Vec3::X, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(0.5, 0.5, 0.5), Vec3::X, Vec2::new(0.0, 0.0)),
            // Left face
            Vertex::new(Vec3::new(-0.5, -0.5, -0.5), -Vec3::X, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, -0.5, 0.5), -Vec3::X, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, 0.5), -Vec3::X, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-0.5, 0.5, -0.5), -Vec3::X, Vec2::new(0.0, 0.0)),
        ];

        let indices = vec![
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
            8, 9, 10, 10, 11, 8, // Top
            12, 13, 14, 14, 15, 12, // Bottom
            16, 17, 18, 18, 19, 16, // Right
            20, 21, 22, 22, 23, 20, // Left
        ];

        Mesh::new("Cube", vertices, indices)
    }

    pub fn create_plane(size: f32, subdivisions: u32) -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let step = size / subdivisions as f32;
        let half = size / 2.0;

        for z in 0..=subdivisions {
            for x in 0..=subdivisions {
                let px = -half + x as f32 * step;
                let pz = -half + z as f32 * step;
                let u = x as f32 / subdivisions as f32;
                let v = z as f32 / subdivisions as f32;

                vertices.push(Vertex::new(
                    Vec3::new(px, 0.0, pz),
                    Vec3::Y,
                    Vec2::new(u, v),
                ));
            }
        }

        for z in 0..subdivisions {
            for x in 0..subdivisions {
                let tl = z * (subdivisions + 1) + x;
                let tr = tl + 1;
                let bl = (z + 1) * (subdivisions + 1) + x;
                let br = bl + 1;

                indices.push(tl);
                indices.push(bl);
                indices.push(tr);

                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }

        Mesh::new("Plane", vertices, indices)
    }

    pub fn create_sphere(segments: u32, rings: u32) -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for ring in 0..=rings {
            let phi = std::f32::consts::PI * ring as f32 / rings as f32;
            let y = phi.cos();
            let sin_phi = phi.sin();

            for seg in 0..=segments {
                let theta = 2.0 * std::f32::consts::PI * seg as f32 / segments as f32;
                let x = theta.cos() * sin_phi;
                let z = theta.sin() * sin_phi;

                let position = Vec3::new(x, y, z);
                let normal = position.normalize();
                let uv = Vec2::new(seg as f32 / segments as f32, ring as f32 / rings as f32);

                vertices.push(Vertex::new(position * 0.5, normal, uv));
            }
        }

        for ring in 0..rings {
            for seg in 0..segments {
                let tl = ring * (segments + 1) + seg;
                let tr = tl + 1;
                let bl = (ring + 1) * (segments + 1) + seg;
                let br = bl + 1;

                indices.push(tl);
                indices.push(bl);
                indices.push(tr);

                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }

        Mesh::new("Sphere", vertices, indices)
    }

    pub fn create_cylinder(segments: u32) -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Top cap
        vertices.push(Vertex::new(
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::Y,
            Vec2::new(0.5, 0.5),
        ));
        for i in 0..=segments {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
            let x = angle.cos() * 0.5;
            let z = angle.sin() * 0.5;
            vertices.push(Vertex::new(
                Vec3::new(x, 0.5, z),
                Vec3::Y,
                Vec2::new(angle.cos() * 0.5 + 0.5, angle.sin() * 0.5 + 0.5),
            ));
        }

        // Bottom cap
        let bottom_center = vertices.len() as u32;
        vertices.push(Vertex::new(
            Vec3::new(0.0, -0.5, 0.0),
            -Vec3::Y,
            Vec2::new(0.5, 0.5),
        ));
        for i in 0..=segments {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
            let x = angle.cos() * 0.5;
            let z = angle.sin() * 0.5;
            vertices.push(Vertex::new(
                Vec3::new(x, -0.5, z),
                -Vec3::Y,
                Vec2::new(angle.cos() * 0.5 + 0.5, angle.sin() * 0.5 + 0.5),
            ));
        }

        // Top cap indices
        for i in 0..segments {
            indices.push(0);
            indices.push(1 + i);
            indices.push(1 + i + 1);
        }

        // Bottom cap indices
        for i in 0..segments {
            indices.push(bottom_center);
            indices.push(bottom_center + 1 + i + 1);
            indices.push(bottom_center + 1 + i);
        }

        // Side indices
        let side_start = 1;
        let side_start_bottom = bottom_center + 1;
        for i in 0..segments {
            let tl = side_start + i;
            let tr = tl + 1;
            let bl = side_start_bottom + i;
            let br = bl + 1;

            indices.push(tl);
            indices.push(bl);
            indices.push(tr);

            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }

        Mesh::new("Cylinder", vertices, indices)
    }

    pub fn create_rifle() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.16, 0.075, 0.30),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.035, -0.46),
            Vec3::new(0.045, 0.045, 0.27),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.035, -0.76),
            Vec3::new(0.06, 0.055, 0.045),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.02, 0.35),
            Vec3::new(0.135, 0.06, 0.18),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.17, 0.08),
            Vec3::new(0.055, 0.15, 0.07),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.18, -0.09),
            Vec3::new(0.075, 0.16, 0.055),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.105, -0.08),
            Vec3::new(0.13, 0.022, 0.24),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.12, -0.62),
            Vec3::new(0.035, 0.08, 0.025),
        );

        Mesh::new("Procedural Rifle", vertices, indices)
    }

    pub fn create_checkerboard_texture(size: u32, grid_size: u32) -> super::Texture {
        let mut data = Vec::with_capacity((size * size * 4) as usize);

        for y in 0..size {
            for x in 0..size {
                let is_white = ((x / grid_size) + (y / grid_size)).is_multiple_of(2);
                let color = if is_white { 255u8 } else { 0u8 };
                data.push(color);
                data.push(color);
                data.push(color);
                data.push(255);
            }
        }

        super::Texture::from_rgba8("Checkerboard", size, size, data)
    }

    pub fn create_flat_normal_texture(size: u32) -> super::Texture {
        let mut data = Vec::with_capacity((size * size * 4) as usize);

        for _ in 0..(size * size) {
            data.push(128); // R
            data.push(128); // G
            data.push(255); // B
            data.push(255); // A
        }

        super::Texture::from_rgba8("FlatNormal", size, size, data)
    }
}

fn add_box(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, center: Vec3, half: Vec3) {
    let base = vertices.len() as u32;
    let min = center - half;
    let max = center + half;
    let corners = [
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
    ];
    let faces = [
        ([0, 1, 2, 3], Vec3::Z),
        ([4, 5, 6, 7], -Vec3::Z),
        ([3, 2, 7, 6], Vec3::Y),
        ([5, 4, 1, 0], -Vec3::Y),
        ([1, 4, 7, 2], Vec3::X),
        ([5, 0, 3, 6], -Vec3::X),
    ];

    for (face_index, (face, normal)) in faces.iter().enumerate() {
        let vertex_base = base + face_index as u32 * 4;
        vertices.push(Vertex::new(corners[face[0]], *normal, Vec2::new(0.0, 1.0)));
        vertices.push(Vertex::new(corners[face[1]], *normal, Vec2::new(1.0, 1.0)));
        vertices.push(Vertex::new(corners[face[2]], *normal, Vec2::new(1.0, 0.0)));
        vertices.push(Vertex::new(corners[face[3]], *normal, Vec2::new(0.0, 0.0)));
        indices.extend_from_slice(&[
            vertex_base,
            vertex_base + 1,
            vertex_base + 2,
            vertex_base + 2,
            vertex_base + 3,
            vertex_base,
        ]);
    }
}
