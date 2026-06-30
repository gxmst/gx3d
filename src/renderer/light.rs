use glam::Vec3;

#[derive(Debug, Clone)]
pub struct Light {
    pub position: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub light_type: LightType,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
}

impl Light {
    pub fn directional(direction: Vec3, color: [f32; 3], intensity: f32) -> Self {
        Self {
            position: -direction.normalize() * 100.0,
            color,
            intensity,
            light_type: LightType::Directional,
            range: f32::MAX,
            inner_angle: 0.0,
            outer_angle: 0.0,
        }
    }

    pub fn point(position: Vec3, color: [f32; 3], intensity: f32, range: f32) -> Self {
        Self {
            position,
            color,
            intensity,
            light_type: LightType::Point,
            range,
            inner_angle: 0.0,
            outer_angle: 0.0,
        }
    }

    pub fn spot(
        position: Vec3,
        _direction: Vec3,
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        Self {
            position,
            color,
            intensity,
            light_type: LightType::Spot,
            range,
            inner_angle,
            outer_angle,
        }
    }
}

impl Default for Light {
    fn default() -> Self {
        Self::directional(Vec3::new(1.0, -1.0, 0.5), [1.0, 1.0, 1.0], 1.0)
    }
}
