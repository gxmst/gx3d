use glam::Vec2;

#[derive(Debug, Clone)]
pub struct RecoilSystem {
    pub mass: f32,
    pub damping: f32,
    pub stiffness: f32,
    pub current_offset: Vec2,
    pub velocity: Vec2,
    pub impulse_queue: Vec<Vec2>,
    pub max_offset: Vec2,
}

impl RecoilSystem {
    pub fn new(mass: f32, damping: f32, stiffness: f32) -> Self {
        Self {
            mass,
            damping,
            stiffness,
            current_offset: Vec2::ZERO,
            velocity: Vec2::ZERO,
            impulse_queue: Vec::new(),
            max_offset: Vec2::new(0.5, 0.3),
        }
    }

    pub fn apply_impulse(&mut self, impulse: Vec2) {
        self.impulse_queue.push(impulse);
    }

    pub fn update(&mut self, dt: f32) {
        // Process impulse queue
        for impulse in self.impulse_queue.drain(..) {
            self.velocity += impulse / self.mass;
        }

        // Spring-damper integration
        let spring_force = -self.stiffness * self.current_offset;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;

        self.velocity += acceleration * dt;
        self.current_offset += self.velocity * dt;

        // Clamp offset
        self.current_offset = self.current_offset.clamp(-self.max_offset, self.max_offset);
    }

    pub fn reset(&mut self) {
        self.current_offset = Vec2::ZERO;
        self.velocity = Vec2::ZERO;
        self.impulse_queue.clear();
    }

    pub fn offset(&self) -> Vec2 {
        self.current_offset
    }
}

impl Default for RecoilSystem {
    fn default() -> Self {
        Self::new(1.0, 8.0, 15.0)
    }
}
