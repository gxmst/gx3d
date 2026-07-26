//! Physics showcase effects: tornado force field, buoyant water surface,
//! and destructible block structures.
//!
//! The water wave shape lives here as plain Rust functions used by BOTH the
//! surface mesh animation and the buoyancy forces, so the two can never drift
//! apart (same convention as the shader constant sync tests).

use glam::{Vec2, Vec3};

/// A vortex force field. One entity per tornado; forces are applied to
/// dynamic bodies each fixed tick, dust motes are purely visual.
#[derive(Debug, Clone)]
pub struct Tornado {
    /// Authored base position of the funnel axis (ground level).
    pub center: Vec3,
    pub radius: f32,
    pub height: f32,
    /// Peak force in newtons near the core.
    pub strength: f32,
    /// How far the funnel base wanders from `center`.
    pub wander: f32,
    /// Simulation-time clock (respects pause / time scale).
    pub time: f32,
}

impl Tornado {
    /// Current funnel base position: the authored center plus a slow
    /// figure-eight wander so props keep getting picked up and dropped.
    pub fn axis_base(&self) -> Vec3 {
        let t = self.time * 0.16;
        self.center
            + Vec3::new(
                (t * 1.0).sin() * self.wander,
                0.0,
                (t * 1.7).sin() * self.wander * 0.7,
            )
    }
}

/// Marks a purely visual dust mote orbiting a tornado funnel.
#[derive(Debug, Clone, Copy)]
pub struct TornadoDust {
    /// Per-mote phase/height seed in [0, 1).
    pub seed: f32,
}

/// An animated water surface with matching buoyancy.
pub struct WaterSurface {
    pub mesh: crate::asset::Handle<crate::asset::Mesh>,
    /// Center of the surface; `center.y` is the rest water level.
    pub center: Vec3,
    /// Edge length of the square surface.
    pub size: f32,
    pub amplitude: f32,
    /// Simulation-time clock (respects pause / time scale).
    pub time: f32,
    /// Undisplaced grid vertices, cloned into the mesh each frame.
    pub base_vertices: Vec<crate::asset::Vertex>,
}

/// A block belonging to a destructible structure. Spawned as a frozen
/// (Fixed) dynamic body; explosions, bullet hits, and losing support
/// unfreeze it so it falls and tumbles.
#[derive(Debug, Clone, Copy)]
pub struct DestructibleBlock {
    /// Vertical half-extent, used by the support raycast length.
    pub half_height: f32,
}

/// Directional component waves: (direction, wavelength m, speed m/s,
/// fraction of the authored amplitude).
const WAVES: [(Vec2, f32, f32, f32); 4] = [
    (Vec2::new(1.0, 0.25), 21.0, 3.2, 0.42),
    (Vec2::new(-0.4, 1.0), 13.0, 2.4, 0.28),
    (Vec2::new(0.8, -0.6), 7.0, 1.9, 0.19),
    (Vec2::new(-0.9, -0.35), 3.4, 1.4, 0.11),
];

/// Water surface height offset at (x, z), relative to the rest level.
pub fn wave_height(x: f32, z: f32, time: f32, amplitude: f32) -> f32 {
    let mut height = 0.0;
    for (direction, wavelength, speed, fraction) in WAVES {
        let k = std::f32::consts::TAU / wavelength;
        let d = direction.normalize();
        let phase = (d.x * x + d.y * z) * k + time * speed * k;
        height += amplitude * fraction * phase.sin();
    }
    height
}

/// Gradient (dh/dx, dh/dz) of [`wave_height`], used for surface normals and
/// the lateral push on floating bodies.
pub fn wave_gradient(x: f32, z: f32, time: f32, amplitude: f32) -> Vec2 {
    let mut gradient = Vec2::ZERO;
    for (direction, wavelength, speed, fraction) in WAVES {
        let k = std::f32::consts::TAU / wavelength;
        let d = direction.normalize();
        let phase = (d.x * x + d.y * z) * k + time * speed * k;
        gradient += d * (amplitude * fraction * k * phase.cos());
    }
    gradient
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_height_stays_within_amplitude() {
        for i in 0..500 {
            let x = (i as f32) * 1.7 - 400.0;
            let z = (i as f32) * -2.3 + 300.0;
            let h = wave_height(x, z, i as f32 * 0.1, 1.0);
            // Component fractions sum to 1.0, so |h| can never exceed the
            // authored amplitude.
            assert!(h.abs() <= 1.0 + 1e-4, "wave height {h} exceeds amplitude");
        }
    }

    #[test]
    fn wave_gradient_matches_finite_difference() {
        let (x, z, t, a) = (3.2, -7.9, 5.0, 0.8);
        let eps = 1e-3;
        let grad = wave_gradient(x, z, t, a);
        let dx = (wave_height(x + eps, z, t, a) - wave_height(x - eps, z, t, a)) / (2.0 * eps);
        let dz = (wave_height(x, z + eps, t, a) - wave_height(x, z - eps, t, a)) / (2.0 * eps);
        assert!((grad.x - dx).abs() < 1e-2);
        assert!((grad.y - dz).abs() < 1e-2);
    }

    #[test]
    fn tornado_wander_stays_within_bounds() {
        let mut tornado = Tornado {
            center: Vec3::new(10.0, 0.0, -5.0),
            radius: 12.0,
            height: 28.0,
            strength: 260.0,
            wander: 6.0,
            time: 0.0,
        };
        for step in 0..2000 {
            tornado.time = step as f32 * 0.25;
            let base = tornado.axis_base();
            assert!((base - tornado.center).length() <= tornado.wander * 1.5);
            assert_eq!(base.y, tornado.center.y);
        }
    }
}
