use glam::Quat;

#[derive(Debug, Clone, Copy)]
pub struct ToggleDoor {
    pub closed_rotation: Quat,
    pub open_rotation: Quat,
    pub open: bool,
    pub progress: f32,
    pub speed: f32,
}

#[derive(Debug, Clone, Default)]
pub struct InteractionFocus {
    pub entity: Option<hecs::Entity>,
    pub title: String,
    pub prompt: String,
    pub distance: f32,
    pub hold_seconds: f32,
}
