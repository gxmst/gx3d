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
    single_step_requests: u32,
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
            single_step_requests: 0,
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last_frame;
        self.last_frame = now;
        self.elapsed = now - self.start_time;
        self.accumulator += self.delta_seconds();
    }

    /// Wall-clock delta, unaffected by simulation speed. UI and free-camera
    /// controls use this so they remain responsive while physics is paused.
    pub fn real_delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32() * self.time_scale
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    pub fn should_fixed_update(&mut self) -> bool {
        if self.single_step_requests > 0 {
            self.single_step_requests -= 1;
            return true;
        }
        if self.accumulator >= self.fixed_timestep {
            self.accumulator -= self.fixed_timestep;
            true
        } else {
            false
        }
    }

    /// Drop whole fixed ticks left after the per-frame safety cap while
    /// preserving the fractional remainder for the next frame.
    pub(crate) fn discard_fixed_update_backlog(&mut self) {
        if self.fixed_timestep.is_finite()
            && self.fixed_timestep > 0.0
            && self.accumulator >= self.fixed_timestep
        {
            self.accumulator %= self.fixed_timestep;
        }
    }

    pub fn request_single_step(&mut self) {
        self.single_step_requests = self.single_step_requests.saturating_add(1);
    }

    pub fn is_paused(&self) -> bool {
        self.time_scale <= f32::EPSILON
    }

    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = if scale.is_finite() {
            scale.clamp(0.0, 4.0)
        } else {
            1.0
        };
        if self.is_paused() {
            self.accumulator = 0.0;
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

#[cfg(test)]
mod tests {
    use super::Time;

    #[test]
    fn pause_clears_pending_ticks_and_single_step_runs_exactly_once() {
        let mut time = Time::new();
        time.accumulator = time.fixed_timestep * 3.0;
        time.set_time_scale(0.0);
        assert!(!time.should_fixed_update());

        time.request_single_step();
        assert!(time.should_fixed_update());
        assert!(!time.should_fixed_update());
    }

    #[test]
    fn fixed_timestep_does_not_change_with_time_scale() {
        let mut time = Time::new();
        let fixed = time.fixed_timestep;
        time.set_time_scale(0.1);
        assert_eq!(time.fixed_timestep, fixed);
        time.set_time_scale(2.0);
        assert_eq!(time.fixed_timestep, fixed);
    }

    #[test]
    fn invalid_time_scale_falls_back_to_normal_speed() {
        let mut time = Time::new();
        time.set_time_scale(f32::NAN);
        assert_eq!(time.time_scale, 1.0);
        time.set_time_scale(f32::INFINITY);
        assert_eq!(time.time_scale, 1.0);
    }

    #[test]
    fn fixed_update_backlog_keeps_only_fractional_remainder() {
        let mut time = Time::new();
        time.accumulator = time.fixed_timestep * 20.5;
        for _ in 0..8 {
            assert!(time.should_fixed_update());
        }

        time.discard_fixed_update_backlog();

        assert!(!time.should_fixed_update());
        assert!(time.accumulator < time.fixed_timestep);
    }
}
