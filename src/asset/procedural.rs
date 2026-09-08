use super::{Mesh, Vertex};
use glam::{Vec2, Vec3};

pub struct ProceduralGenerator;

const MIN_DIMENSION: f32 = 0.001;
const MAX_DIMENSION: f32 = 10_000.0;
const DEFAULT_PLANE_SIZE: f32 = 1.0;
const DEFAULT_SPHERE_RADIUS: f32 = 0.5;
const DEFAULT_CYLINDER_RADIUS: f32 = 0.5;
const DEFAULT_CYLINDER_HEIGHT: f32 = 1.0;

fn safe_dimension(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value.clamp(MIN_DIMENSION, MAX_DIMENSION)
    } else {
        fallback
    }
}

fn safe_plane_subdivisions(subdivisions: u32) -> u32 {
    subdivisions.clamp(1, 512)
}

fn safe_sphere_segments(segments: u32) -> u32 {
    segments.clamp(3, 256)
}

fn safe_sphere_rings(rings: u32) -> u32 {
    rings.clamp(2, 128)
}

fn safe_cylinder_segments(segments: u32) -> u32 {
    segments.clamp(3, 256)
}

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
        let size = safe_dimension(size, DEFAULT_PLANE_SIZE);
        let subdivisions = safe_plane_subdivisions(subdivisions);
        let vertices_per_side = subdivisions as usize + 1;
        let mut vertices = Vec::with_capacity(vertices_per_side * vertices_per_side);
        let mut indices = Vec::with_capacity(subdivisions as usize * subdivisions as usize * 6);

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
        Self::create_sphere_with_radius(DEFAULT_SPHERE_RADIUS, segments, rings)
    }

    /// Create a UV sphere while constraining user-authored dimensions and
    /// tessellation to finite, useful values. The regular `create_sphere`
    /// entry point keeps the historical unit-diameter behavior.
    pub fn create_sphere_with_radius(radius: f32, segments: u32, rings: u32) -> Mesh {
        let radius = safe_dimension(radius, DEFAULT_SPHERE_RADIUS);
        let segments = safe_sphere_segments(segments);
        let rings = safe_sphere_rings(rings);
        let mut vertices = Vec::with_capacity((segments as usize + 1) * (rings as usize + 1));
        let mut indices = Vec::with_capacity(segments as usize * rings as usize * 6);

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

                vertices.push(Vertex::new(position * radius, normal, uv));
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
        Self::create_cylinder_with_dimensions(
            DEFAULT_CYLINDER_RADIUS,
            DEFAULT_CYLINDER_HEIGHT,
            segments,
        )
    }

    /// Create a Y-aligned cylinder with guarded dimensions. Keeping this
    /// validation at the generator boundary protects every current and future
    /// caller, including data-driven scenes.
    pub fn create_cylinder_with_dimensions(radius: f32, height: f32, segments: u32) -> Mesh {
        let radius = safe_dimension(radius, DEFAULT_CYLINDER_RADIUS);
        let half_height = safe_dimension(height, DEFAULT_CYLINDER_HEIGHT) * 0.5;
        let segments = safe_cylinder_segments(segments);
        let mut vertices = Vec::with_capacity(4 * segments as usize + 6);
        let mut indices = Vec::with_capacity(12 * segments as usize);

        // Caps use their own vertices so their normals stay vertical.
        let top_center = vertices.len() as u32;
        vertices.push(Vertex::new(
            Vec3::new(0.0, half_height, 0.0),
            Vec3::Y,
            Vec2::splat(0.5),
        ));
        for i in 0..=segments {
            let a = std::f32::consts::TAU * i as f32 / segments as f32;
            vertices.push(Vertex::new(
                Vec3::new(a.cos() * radius, half_height, a.sin() * radius),
                Vec3::Y,
                Vec2::new(a.cos() * 0.5 + 0.5, a.sin() * 0.5 + 0.5),
            ));
        }
        for i in 0..segments {
            indices.extend_from_slice(&[top_center, top_center + i + 2, top_center + i + 1]);
        }

        let bottom_center = vertices.len() as u32;
        vertices.push(Vertex::new(
            Vec3::new(0.0, -half_height, 0.0),
            -Vec3::Y,
            Vec2::splat(0.5),
        ));
        for i in 0..=segments {
            let a = std::f32::consts::TAU * i as f32 / segments as f32;
            vertices.push(Vertex::new(
                Vec3::new(a.cos() * radius, -half_height, a.sin() * radius),
                -Vec3::Y,
                Vec2::new(a.cos() * 0.5 + 0.5, a.sin() * 0.5 + 0.5),
            ));
        }
        for i in 0..segments {
            indices.extend_from_slice(&[
                bottom_center,
                bottom_center + i + 1,
                bottom_center + i + 2,
            ]);
        }

        // Sides need radial normals and independent seam vertices. The old
        // mesh reused cap normals here, causing black/transparent-looking oil barrels.
        let side_start = vertices.len() as u32;
        for i in 0..=segments {
            let a = std::f32::consts::TAU * i as f32 / segments as f32;
            let normal = Vec3::new(a.cos(), 0.0, a.sin());
            let u = i as f32 / segments as f32;
            vertices.push(Vertex::new(
                Vec3::new(normal.x * radius, half_height, normal.z * radius),
                normal,
                Vec2::new(u, 0.0),
            ));
            vertices.push(Vertex::new(
                Vec3::new(normal.x * radius, -half_height, normal.z * radius),
                normal,
                Vec2::new(u, 1.0),
            ));
        }
        for i in 0..segments {
            let top_left = side_start + i * 2;
            let bottom_left = top_left + 1;
            let top_right = top_left + 2;
            let bottom_right = top_left + 3;
            indices.extend_from_slice(&[
                top_left,
                top_right,
                bottom_left,
                top_right,
                bottom_right,
                bottom_left,
            ]);
        }

        Mesh::new("Cylinder", vertices, indices)
    }

    /// Compact sidearm: short slide over a grip frame, stubby barrel.
    pub fn create_pistol() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        // Slide (top) and frame (below it).
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.05, -0.05),
            Vec3::new(0.075, 0.05, 0.24),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, -0.02),
            Vec3::new(0.065, 0.03, 0.19),
        );
        // Short exposed muzzle.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.05, -0.33),
            Vec3::new(0.035, 0.035, 0.05),
        );
        // Raked grip with a magazine base poking out.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.15, 0.12),
            Vec3::new(0.06, 0.13, 0.075),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.285, 0.13),
            Vec3::new(0.07, 0.02, 0.085),
        );
        // Trigger guard loop (front bar + bottom bar).
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.075, -0.045),
            Vec3::new(0.02, 0.055, 0.018),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.125, 0.015),
            Vec3::new(0.02, 0.014, 0.08),
        );
        // Front/rear sights.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.115, -0.26),
            Vec3::new(0.012, 0.018, 0.012),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.115, 0.16),
            Vec3::new(0.03, 0.018, 0.014),
        );
        Mesh::new("Pistol", vertices, indices)
    }

    /// Stubby SMG: boxy receiver, long magazine well forward of the grip,
    /// wire-frame style stock silhouette.
    pub fn create_smg() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        // Receiver.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.02, -0.08),
            Vec3::new(0.09, 0.07, 0.30),
        );
        // Short barrel with a chunky shroud.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.03, -0.44),
            Vec3::new(0.05, 0.05, 0.09),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.03, -0.56),
            Vec3::new(0.028, 0.028, 0.06),
        );
        // Long stick magazine angled slightly forward, ahead of the grip.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.17, -0.16),
            Vec3::new(0.038, 0.16, 0.055),
        );
        // Pistol grip.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.14, 0.14),
            Vec3::new(0.05, 0.12, 0.06),
        );
        // Skeleton stock: two thin bars meeting a butt plate.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.05, 0.32),
            Vec3::new(0.022, 0.022, 0.13),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, 0.44),
            Vec3::new(0.022, 0.09, 0.022),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, 0.47),
            Vec3::new(0.05, 0.11, 0.02),
        );
        // Sights.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.11, -0.38),
            Vec3::new(0.014, 0.022, 0.014),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.11, 0.1),
            Vec3::new(0.04, 0.022, 0.016),
        );
        Mesh::new("SMG", vertices, indices)
    }

    /// Long marksman rifle: extended heavy barrel, scope tube on tall rings,
    /// full stock with a cheek riser.
    pub fn create_marksman_rifle() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        // Receiver.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.0, 0.02),
            Vec3::new(0.08, 0.065, 0.26),
        );
        // Long tapered barrel (two segments) + muzzle brake.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.02, -0.52),
            Vec3::new(0.035, 0.035, 0.30),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.02, -0.86),
            Vec3::new(0.028, 0.028, 0.10),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.02, -0.98),
            Vec3::new(0.045, 0.045, 0.035),
        );
        // Scope: main tube + objective bell + two mounting rings.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.145, -0.10),
            Vec3::new(0.038, 0.038, 0.17),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.145, -0.30),
            Vec3::new(0.05, 0.05, 0.045),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.09, -0.16),
            Vec3::new(0.02, 0.03, 0.02),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.09, 0.0),
            Vec3::new(0.02, 0.03, 0.02),
        );
        // Box magazine.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.12, -0.08),
            Vec3::new(0.04, 0.075, 0.06),
        );
        // Grip and full stock with cheek riser + butt.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.14, 0.15),
            Vec3::new(0.05, 0.12, 0.06),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.01, 0.38),
            Vec3::new(0.055, 0.05, 0.20),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.065, 0.42),
            Vec3::new(0.045, 0.025, 0.12),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.03, 0.585),
            Vec3::new(0.06, 0.10, 0.025),
        );
        Mesh::new("Marksman Rifle", vertices, indices)
    }

    /// Pump shotgun: fat receiver, tube magazine under the barrel, pump
    /// foregrip, shoulder stock.
    pub fn create_shotgun() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        // Receiver.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.01, 0.05),
            Vec3::new(0.085, 0.075, 0.22),
        );
        // Barrel and the parallel tube magazine right under it.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.05, -0.45),
            Vec3::new(0.032, 0.032, 0.34),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, -0.42),
            Vec3::new(0.028, 0.028, 0.30),
        );
        // Pump foregrip sleeve around the mag tube.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, -0.33),
            Vec3::new(0.055, 0.05, 0.10),
        );
        // Bead sight at the muzzle.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.095, -0.77),
            Vec3::new(0.012, 0.014, 0.012),
        );
        // Grip + broad stock.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.13, 0.20),
            Vec3::new(0.05, 0.11, 0.065),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.02, 0.40),
            Vec3::new(0.06, 0.06, 0.17),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.035, 0.575),
            Vec3::new(0.065, 0.105, 0.025),
        );
        Mesh::new("Shotgun", vertices, indices)
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
        // Front sight, rear sight and top rail silhouette.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.155, -0.67),
            Vec3::new(0.018, 0.045, 0.018),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.142, 0.05),
            Vec3::new(0.055, 0.035, 0.035),
        );
        for z in [-0.48, -0.36, -0.24, -0.12, 0.0] {
            add_box(
                &mut vertices,
                &mut indices,
                Vec3::new(0.0, 0.128, z),
                Vec3::new(0.12, 0.012, 0.025),
            );
        }
        // Handguard side ribs and stock details break up the slab-like form.
        for z in [-0.52, -0.40, -0.28] {
            add_box(
                &mut vertices,
                &mut indices,
                Vec3::new(0.0, 0.0, z),
                Vec3::new(0.07, 0.065, 0.032),
            );
        }
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.055, 0.48),
            Vec3::new(0.11, 0.085, 0.10),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.28, -0.08),
            Vec3::new(0.065, 0.05, 0.085),
        );

        Mesh::new("Procedural Rifle", vertices, indices)
    }

    pub fn create_wedge() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let p = [
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(0.5, -0.5, -0.5),
            Vec3::new(-0.5, -0.5, 0.5),
            Vec3::new(0.5, -0.5, 0.5),
            Vec3::new(-0.5, 0.5, 0.5),
            Vec3::new(0.5, 0.5, 0.5),
        ];
        let mut face = |points: &[Vec3], normal: Vec3| {
            let base = vertices.len() as u32;
            for (index, point) in points.iter().enumerate() {
                let uv = match index {
                    0 => Vec2::new(0.0, 1.0),
                    1 => Vec2::new(1.0, 1.0),
                    2 => Vec2::new(1.0, 0.0),
                    _ => Vec2::new(0.0, 0.0),
                };
                vertices.push(Vertex::new(*point, normal, uv));
            }
            if points.len() == 3 {
                indices.extend_from_slice(&[base, base + 1, base + 2]);
            } else {
                indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
            }
        };
        face(&[p[0], p[1], p[3], p[2]], -Vec3::Y);
        face(&[p[2], p[3], p[5], p[4]], Vec3::Z);
        let slope_normal = Vec3::new(0.0, 1.0, -1.0).normalize();
        face(&[p[0], p[4], p[5], p[1]], slope_normal);
        face(&[p[0], p[2], p[4]], -Vec3::X);
        face(&[p[1], p[5], p[3]], Vec3::X);
        Mesh::new("Wedge", vertices, indices)
    }

    pub fn create_humanoid() -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        // Boots and separated legs.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(-0.20, -0.86, 0.02),
            Vec3::new(0.14, 0.42, 0.16),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.20, -0.86, 0.02),
            Vec3::new(0.14, 0.42, 0.16),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(-0.20, -1.25, -0.08),
            Vec3::new(0.16, 0.10, 0.28),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.20, -1.25, -0.08),
            Vec3::new(0.16, 0.10, 0.28),
        );
        // Pelvis, torso, chest and neck create a readable silhouette.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, -0.34, 0.0),
            Vec3::new(0.36, 0.22, 0.22),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.12, 0.0),
            Vec3::new(0.42, 0.36, 0.24),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.48, 0.0),
            Vec3::new(0.50, 0.16, 0.27),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.72, 0.0),
            Vec3::new(0.13, 0.12, 0.13),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.0, 0.98, 0.0),
            Vec3::new(0.25, 0.27, 0.24),
        );
        // Arms, forearms and hands.
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(-0.58, 0.25, 0.0),
            Vec3::new(0.13, 0.38, 0.15),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.58, 0.25, 0.0),
            Vec3::new(0.13, 0.38, 0.15),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(-0.60, -0.18, -0.03),
            Vec3::new(0.12, 0.28, 0.13),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.60, -0.18, -0.03),
            Vec3::new(0.12, 0.28, 0.13),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(-0.60, -0.50, -0.08),
            Vec3::new(0.14, 0.12, 0.16),
        );
        add_box(
            &mut vertices,
            &mut indices,
            Vec3::new(0.60, -0.50, -0.08),
            Vec3::new(0.14, 0.12, 0.16),
        );
        Mesh::new("Humanoid", vertices, indices)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_valid_mesh(mesh: &Mesh) {
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert!(mesh
            .indices
            .iter()
            .all(|index| *index < mesh.vertices.len() as u32));
        assert!(mesh.vertices.iter().all(|vertex| {
            vertex.position.iter().all(|value| value.is_finite())
                && vertex.normal.iter().all(|value| value.is_finite())
                && vertex.uv.iter().all(|value| value.is_finite())
        }));
    }

    #[test]
    fn tessellation_parameters_are_bounded() {
        assert_eq!(safe_plane_subdivisions(0), 1);
        assert_eq!(safe_plane_subdivisions(u32::MAX), 512);
        assert_eq!(safe_sphere_segments(0), 3);
        assert_eq!(safe_sphere_segments(u32::MAX), 256);
        assert_eq!(safe_sphere_rings(0), 2);
        assert_eq!(safe_sphere_rings(u32::MAX), 128);
        assert_eq!(safe_cylinder_segments(0), 3);
        assert_eq!(safe_cylinder_segments(u32::MAX), 256);
    }

    #[test]
    fn invalid_dimensions_use_safe_finite_values() {
        assert_eq!(safe_dimension(f32::NAN, 1.0), 1.0);
        assert_eq!(safe_dimension(f32::INFINITY, 1.0), 1.0);
        assert_eq!(safe_dimension(0.0, 1.0), 1.0);
        assert_eq!(safe_dimension(-3.0, 1.0), 1.0);
        assert_eq!(safe_dimension(f32::MAX, 1.0), MAX_DIMENSION);
        assert_eq!(safe_dimension(f32::MIN_POSITIVE, 1.0), MIN_DIMENSION);
    }

    #[test]
    fn plane_with_invalid_input_remains_renderable() {
        let mesh = ProceduralGenerator::create_plane(f32::NAN, 0);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        assert_valid_mesh(&mesh);
        assert!(mesh.vertices.iter().any(|vertex| vertex.position[0] != 0.0));
        assert!(mesh.vertices.iter().any(|vertex| vertex.position[2] != 0.0));
    }

    #[test]
    fn sphere_with_invalid_input_remains_renderable() {
        let mesh = ProceduralGenerator::create_sphere_with_radius(f32::NEG_INFINITY, 0, 0);

        assert_eq!(mesh.vertices.len(), 12);
        assert_eq!(mesh.indices.len(), 36);
        assert_valid_mesh(&mesh);
    }

    #[test]
    fn cylinder_with_invalid_input_remains_renderable() {
        let mesh = ProceduralGenerator::create_cylinder_with_dimensions(f32::NAN, 0.0, 0);

        assert_eq!(mesh.vertices.len(), 18);
        assert_eq!(mesh.indices.len(), 36);
        assert_valid_mesh(&mesh);
        assert!(mesh.vertices.iter().any(|vertex| vertex.position[1] > 0.0));
        assert!(mesh.vertices.iter().any(|vertex| vertex.position[1] < 0.0));
    }
}
