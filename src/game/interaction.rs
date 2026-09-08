use glam::{Quat, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct ToggleDoor {
    pub closed_position: Vec3,
    pub closed_rotation: Quat,
    pub open_rotation: Quat,
    /// Local-space vector from the door centre to its hinge.
    pub hinge_offset: Vec3,
    pub open: bool,
    pub progress: f32,
    pub speed: f32,
}

impl ToggleDoor {
    pub fn pose_at(&self, progress: f32) -> (Vec3, Quat) {
        let progress = progress.clamp(0.0, 1.0);
        let rotation = self.closed_rotation.slerp(self.open_rotation, progress);
        let hinge = self.closed_position + self.closed_rotation * self.hinge_offset;
        let position = hinge - rotation * self.hinge_offset;
        (position, rotation)
    }
}

#[cfg(test)]
mod tests {
    use super::ToggleDoor;
    use glam::{Quat, Vec3};

    #[test]
    fn door_pose_keeps_authored_hinge_fixed() {
        let door = ToggleDoor {
            closed_position: Vec3::new(3.0, 1.5, -2.0),
            closed_rotation: Quat::from_rotation_y(0.2),
            open_rotation: Quat::from_rotation_y(1.5),
            hinge_offset: Vec3::new(-1.0, 0.0, 0.0),
            open: false,
            progress: 0.0,
            speed: 4.0,
        };
        let closed_hinge = door.closed_position + door.closed_rotation * door.hinge_offset;
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let (position, rotation) = door.pose_at(progress);
            let hinge = position + rotation * door.hinge_offset;
            assert!(hinge.distance(closed_hinge) < 1.0e-5);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Explosive {
    pub radius: f32,
    pub impulse: f32,
}

#[derive(Debug, Clone, Default)]
pub struct InteractionFocus {
    pub entity: Option<hecs::Entity>,
    pub title: String,
    pub prompt: String,
    pub distance: f32,
    pub hold_seconds: f32,
}
