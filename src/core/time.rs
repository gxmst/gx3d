use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Time {
    pub delta: Duration,
    pub elapsed: Duration,
    pub fixed_timestep: f32,
    pub time_scale: f32,
    last_frame: Instant,
    start_time: Instant,
    accumulator: f32,
}

impl Time {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            fixed_timestep: 1.0 / 60.0,
            time_scale: 1.0,
            last_frame: now,
            start_time: now,
            accumulator: 0.0,
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last_frame;
        self.last_frame = now;
        self.elapsed = now - self.start_time;
        self.accumulator += self.delta_seconds();
    }

    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32() * self.time_scale
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_timestep {
            self.accumulator -= self.fixed_timestep;
            true
        } else {
            false
        }
    }

    pub fn fps(&self) -> f32 {
        if self.delta.as_secs_f32() > 0.0 {
            1.0 / self.delta.as_secs_f32()
        } else {
            0.0
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}
