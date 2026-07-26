use rapier3d::prelude::{ColliderHandle, RigidBodyHandle};

#[derive(Debug, Clone, Copy)]
pub struct PhysicsBody {
    pub rigid_body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    pub is_static: bool,
}

impl PhysicsBody {
    pub fn new(
        rigid_body_handle: RigidBodyHandle,
        collider_handle: ColliderHandle,
        is_static: bool,
    ) -> Self {
        Self {
            rigid_body_handle,
            collider_handle,
            is_static,
        }
    }
}
