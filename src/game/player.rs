use crate::input::InputState;
use crate::kinematics::CameraController;
use crate::renderer::Camera;
use glam::Vec3;

const JUMP_BUFFER_SECONDS: f32 = 0.12;
const COYOTE_TIME_SECONDS: f32 = 0.10;
const JUMP_SPEED: f32 = 6.8;
const GROUND_ACCELERATION: f32 = 48.0;
const GROUND_DECELERATION: f32 = 58.0;
const AIR_ACCELERATION: f32 = 13.0;
const STICK_TO_GROUND_SPEED: f32 = 1.5;
const TERMINAL_FALL_SPEED: f32 = 45.0;
const FALL_RECOVERY_Y: f32 = -40.0;

pub struct Player {
    pub camera_controller: CameraController,
    pub height: f32,
    pub is_grounded: bool,
    pub(crate) movement_input: Vec3,
    pub(crate) horizontal_velocity: Vec3,
    pub(crate) vertical_velocity: f32,
    sprinting: bool,
    jump_buffer_remaining: f32,
    coyote_time_remaining: f32,
    spawn_position: Vec3,
    last_safe_position: Vec3,
}

impl Player {
    pub fn new() -> Self {
        Self {
            camera_controller: CameraController::default(),
            height: 1.6,
            is_grounded: false,
            movement_input: Vec3::ZERO,
            horizontal_velocity: Vec3::ZERO,
            vertical_velocity: 0.0,
            sprinting: false,
            jump_buffer_remaining: 0.0,
            coyote_time_remaining: 0.0,
            spawn_position: Vec3::ZERO,
            last_safe_position: Vec3::ZERO,
        }
    }

    pub fn update(&mut self, camera: &mut Camera, input: &InputState, dt: f32) {
        self.camera_controller.update(camera, input, dt);
    }

    /// Set the checkpoint used when the player falls out of the authored map.
    /// The latest stable grounded position will replace it during play.
    pub fn set_spawn_position(&mut self, position: Vec3) {
        if position.is_finite() {
            self.spawn_position = position;
            self.last_safe_position = position;
        }
    }

    pub(crate) fn set_movement_input(&mut self, direction: Vec3, sprinting: bool) {
        self.movement_input = if direction.is_finite() {
            direction.clamp_length_max(1.0)
        } else {
            Vec3::ZERO
        };
        self.sprinting = sprinting;
    }

    pub(crate) fn stop_movement(&mut self) {
        self.movement_input = Vec3::ZERO;
        self.horizontal_velocity = Vec3::ZERO;
        self.sprinting = false;
    }

    /// Buffer a jump edge until the next fixed update. This prevents a short
    /// Space press from being lost on render frames where no physics tick runs.
    pub(crate) fn queue_jump(&mut self) {
        self.jump_buffer_remaining = JUMP_BUFFER_SECONDS;
    }

    /// Advance player-owned velocity state by one fixed physics tick and
    /// return the desired collision-constrained translation for that tick.
    pub(crate) fn desired_translation(&mut self, dt: f32, gravity_y: f32) -> Vec3 {
        if !dt.is_finite() || dt <= 0.0 {
            return Vec3::ZERO;
        }

        self.jump_buffer_remaining = (self.jump_buffer_remaining - dt).max(0.0);
        self.coyote_time_remaining = if self.is_grounded {
            COYOTE_TIME_SECONDS
        } else {
            (self.coyote_time_remaining - dt).max(0.0)
        };

        let speed = self.camera_controller.move_speed
            * if self.sprinting {
                self.camera_controller.sprint_multiplier
            } else {
                1.0
            };
        let target_velocity = self.movement_input * speed.max(0.0);
        let acceleration = if self.is_grounded {
            if self.movement_input.length_squared() > 0.0 {
                GROUND_ACCELERATION
            } else {
                GROUND_DECELERATION
            }
        } else {
            AIR_ACCELERATION
        };
        self.horizontal_velocity =
            move_towards(self.horizontal_velocity, target_velocity, acceleration * dt);

        if self.jump_buffer_remaining > 0.0 && self.coyote_time_remaining > 0.0 {
            self.vertical_velocity = JUMP_SPEED;
            self.jump_buffer_remaining = 0.0;
            self.coyote_time_remaining = 0.0;
            self.is_grounded = false;
        } else if self.is_grounded {
            // A small downward intent lets the character controller snap down
            // shallow stairs instead of hovering for a frame at every edge.
            self.vertical_velocity = -STICK_TO_GROUND_SPEED;
        } else {
            let gravity_y = if gravity_y.is_finite() {
                gravity_y.min(0.0)
            } else {
                -9.81
            };
            self.vertical_velocity =
                (self.vertical_velocity + gravity_y * dt).max(-TERMINAL_FALL_SPEED);
        }

        (self.horizontal_velocity + Vec3::Y * self.vertical_velocity) * dt
    }

    pub(crate) fn finish_character_move(
        &mut self,
        grounded: bool,
        hit_ceiling: bool,
        resulting_position: Vec3,
    ) {
        if hit_ceiling && self.vertical_velocity > 0.0 {
            self.vertical_velocity = 0.0;
        }

        self.is_grounded = grounded && self.vertical_velocity <= 0.0;
        if self.is_grounded {
            self.vertical_velocity = -STICK_TO_GROUND_SPEED;
            if resulting_position.is_finite() && resulting_position.y > FALL_RECOVERY_Y {
                self.last_safe_position = resulting_position;
            }
        }
    }

    pub(crate) fn needs_fall_recovery(&self, position: Vec3) -> bool {
        !position.is_finite() || position.y < FALL_RECOVERY_Y
    }

    pub(crate) fn recovery_position(&self) -> Vec3 {
        if self.last_safe_position.is_finite() {
            self.last_safe_position
        } else {
            self.spawn_position
        }
    }

    pub(crate) fn reset_motion(&mut self) {
        self.horizontal_velocity = Vec3::ZERO;
        self.vertical_velocity = 0.0;
        self.jump_buffer_remaining = 0.0;
        self.coyote_time_remaining = 0.0;
        self.is_grounded = false;
    }

    pub(crate) fn is_rising(&self) -> bool {
        self.vertical_velocity > 0.0
    }
}

fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_delta || distance <= f32::EPSILON {
        target
    } else {
        current + delta / distance * max_delta.max(0.0)
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_jump_request_is_consumed_only_once() {
        let mut player = Player::new();
        player.is_grounded = true;
        player.queue_jump();

        let first = player.desired_translation(1.0 / 60.0, -9.81);
        let first_vertical_velocity = player.vertical_velocity;
        player.finish_character_move(false, false, first);
        let _second = player.desired_translation(1.0 / 60.0, -9.81);

        assert!(first_vertical_velocity > 6.0);
        assert!(player.vertical_velocity < first_vertical_velocity);
        assert_eq!(player.jump_buffer_remaining, 0.0);
    }

    #[test]
    fn coyote_time_allows_a_jump_just_after_leaving_ground() {
        let mut player = Player::new();
        player.is_grounded = true;
        let _ = player.desired_translation(1.0 / 60.0, -9.81);
        player.finish_character_move(false, false, Vec3::ZERO);
        player.queue_jump();

        let _ = player.desired_translation(1.0 / 60.0, -9.81);

        assert!(player.vertical_velocity > 6.0);
        assert!(!player.is_grounded);
    }

    #[test]
    fn horizontal_speed_converges_without_overshooting_the_limit() {
        let mut player = Player::new();
        player.is_grounded = true;
        player.set_movement_input(Vec3::X, true);
        for _ in 0..120 {
            let _ = player.desired_translation(1.0 / 60.0, -9.81);
            player.finish_character_move(true, false, Vec3::ZERO);
        }

        let maximum =
            player.camera_controller.move_speed * player.camera_controller.sprint_multiplier;
        assert!(player.horizontal_velocity.length() <= maximum + 1.0e-4);
        assert!((player.horizontal_velocity.length() - maximum).abs() < 1.0e-3);
    }

    #[test]
    fn invalid_or_deep_positions_trigger_checkpoint_recovery() {
        let mut player = Player::new();
        let checkpoint = Vec3::new(2.0, 3.0, 4.0);
        player.set_spawn_position(checkpoint);

        assert!(player.needs_fall_recovery(Vec3::new(0.0, -50.0, 0.0)));
        assert!(player.needs_fall_recovery(Vec3::splat(f32::NAN)));
        assert_eq!(player.recovery_position(), checkpoint);
    }
}
