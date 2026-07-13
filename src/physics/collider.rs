use glam::Vec3;
use rapier3d::prelude::{Collider, ColliderBuilder};

#[derive(Debug, Clone, Copy)]
pub struct PhysicsMaterial {
    pub friction: f32,
    pub restitution: f32,
}

impl PhysicsMaterial {
    pub const DEFAULT: Self = Self {
        friction: 0.7,
        restitution: 0.05,
    };

    pub const ICE: Self = Self {
        friction: 0.02,
        restitution: 0.0,
    };

    pub const RUBBER: Self = Self {
        friction: 0.9,
        restitution: 0.85,
    };

    pub const METAL: Self = Self {
        friction: 0.35,
        restitution: 0.2,
    };
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PhysicsShape {
    Sphere { radius: f32 },
    Cuboid { half_extents: Vec3 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    Wedge { half_extents: Vec3 },
}

impl PhysicsShape {
    fn builder(&self) -> ColliderBuilder {
        match self {
            PhysicsShape::Sphere { radius } => ColliderBuilder::ball(valid_half_extent(*radius)),
            PhysicsShape::Cuboid { half_extents } => ColliderBuilder::cuboid(
                valid_half_extent(half_extents.x),
                valid_half_extent(half_extents.y),
                valid_half_extent(half_extents.z),
            ),
            PhysicsShape::Capsule {
                radius,
                half_height,
            } => ColliderBuilder::capsule_y(
                valid_half_extent(*half_height),
                valid_half_extent(*radius),
            ),
            PhysicsShape::Cylinder {
                radius,
                half_height,
            } => ColliderBuilder::cylinder(
                valid_half_extent(*half_height),
                valid_half_extent(*radius),
            ),
            PhysicsShape::Wedge { half_extents: h } => {
                let h = Vec3::new(
                    valid_half_extent(h.x),
                    valid_half_extent(h.y),
                    valid_half_extent(h.z),
                );
                let points = vec![
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(-h.x, -h.y, h.z),
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(-h.x, h.y, h.z),
                    Vec3::new(h.x, h.y, h.z),
                ];
                ColliderBuilder::convex_hull(&points)
                    .unwrap_or_else(|| ColliderBuilder::cuboid(h.x, h.y, h.z))
            }
        }
    }

    pub fn to_rapier_collider(&self) -> Collider {
        self.to_rapier_collider_with_material(PhysicsMaterial::default())
    }

    pub fn to_rapier_collider_with_material(&self, material: PhysicsMaterial) -> Collider {
        let friction = if material.friction.is_finite() {
            material.friction.max(0.0)
        } else {
            PhysicsMaterial::DEFAULT.friction
        };
        let restitution = if material.restitution.is_finite() {
            material.restitution.clamp(0.0, 1.0)
        } else {
            PhysicsMaterial::DEFAULT.restitution
        };
        self.builder()
            .friction(friction)
            .restitution(restitution)
            .build()
    }
}

fn valid_half_extent(value: f32) -> f32 {
    if value.is_finite() && value.abs() >= 0.001 {
        value.abs().min(10_000.0)
    } else {
        0.001
    }
}

#[cfg(test)]
mod tests {
    use super::PhysicsShape;
    use glam::Vec3;

    #[test]
    fn degenerate_wedge_builds_a_finite_collider() {
        let collider = PhysicsShape::Wedge {
            half_extents: Vec3::new(0.0, f32::NAN, f32::INFINITY),
        }
        .to_rapier_collider();
        let aabb = collider.compute_aabb();
        assert!(aabb.mins.is_finite());
        assert!(aabb.maxs.is_finite());
    }

    #[test]
    fn invalid_primitive_dimensions_and_material_are_sanitized() {
        let shapes = [
            PhysicsShape::Sphere { radius: f32::NAN },
            PhysicsShape::Cuboid {
                half_extents: Vec3::new(-1.0, 0.0, f32::INFINITY),
            },
            PhysicsShape::Capsule {
                radius: -0.4,
                half_height: 0.0,
            },
            PhysicsShape::Cylinder {
                radius: f32::NAN,
                half_height: -1.0,
            },
        ];
        for shape in shapes {
            let collider = shape.to_rapier_collider_with_material(super::PhysicsMaterial {
                friction: f32::NAN,
                restitution: 8.0,
            });
            assert!(collider.compute_aabb().mins.is_finite());
            assert!(collider.compute_aabb().maxs.is_finite());
            assert!(collider.friction().is_finite());
            assert!((0.0..=1.0).contains(&collider.restitution()));
        }

        let huge = PhysicsShape::Sphere { radius: f32::MAX }.to_rapier_collider();
        assert!(huge.compute_aabb().maxs.x <= 10_000.0);
    }
}
