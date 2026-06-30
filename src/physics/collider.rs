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
}

impl PhysicsShape {
    fn builder(&self) -> ColliderBuilder {
        match self {
            PhysicsShape::Sphere { radius } => ColliderBuilder::ball(*radius),
            PhysicsShape::Cuboid { half_extents } => {
                ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z)
            }
            PhysicsShape::Capsule {
                radius,
                half_height,
            } => ColliderBuilder::capsule_y(*half_height, *radius),
            PhysicsShape::Cylinder {
                radius,
                half_height,
            } => ColliderBuilder::cylinder(*half_height, *radius),
        }
    }

    pub fn to_rapier_collider(&self) -> Collider {
        self.to_rapier_collider_with_material(PhysicsMaterial::default())
    }

    pub fn to_rapier_collider_with_material(&self, material: PhysicsMaterial) -> Collider {
        self.builder()
            .friction(material.friction)
            .restitution(material.restitution)
            .build()
    }
}
