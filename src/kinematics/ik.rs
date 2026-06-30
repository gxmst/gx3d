use glam::{Quat, Vec3};

#[derive(Debug, Clone)]
pub struct JointConstraint {
    pub length: f32,
    pub min_angle: f32,
    pub max_angle: f32,
}

impl JointConstraint {
    pub fn new(length: f32) -> Self {
        Self {
            length,
            min_angle: -std::f32::consts::PI,
            max_angle: std::f32::consts::PI,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IKSolver {
    pub max_iterations: u32,
    pub tolerance: f32,
}

impl IKSolver {
    pub fn new(max_iterations: u32, tolerance: f32) -> Self {
        Self {
            max_iterations,
            tolerance,
        }
    }

    pub fn solve(&self, joints: &mut [Vec3], target: Vec3, constraints: &[JointConstraint]) {
        if joints.is_empty() || joints.len() != constraints.len() {
            return;
        }

        let base = joints[0];
        let chain_length = joints.len();

        for _ in 0..self.max_iterations {
            // Forward reaching
            joints[chain_length - 1] = target;
            for i in (1..chain_length).rev() {
                let direction = (joints[i] - joints[i - 1]).normalize_or_zero();
                joints[i - 1] = joints[i] - direction * constraints[i].length;
            }

            // Backward reaching
            joints[0] = base;
            for i in 0..chain_length - 1 {
                let direction = (joints[i + 1] - joints[i]).normalize_or_zero();
                joints[i + 1] = joints[i] + direction * constraints[i + 1].length;
            }

            // Check convergence
            if (joints[chain_length - 1] - target).length() < self.tolerance {
                break;
            }
        }
    }

    pub fn solve_with_rotation(
        &self,
        joints: &mut [Vec3],
        rotations: &mut [Quat],
        target: Vec3,
        constraints: &[JointConstraint],
    ) {
        self.solve(joints, target, constraints);

        // Calculate rotations from joint positions
        for i in 0..joints.len() - 1 {
            let direction = (joints[i + 1] - joints[i]).normalize_or_zero();
            if direction.length_squared() > 0.001 {
                rotations[i] = Quat::from_rotation_arc(Vec3::Y, direction);
            }
        }
    }
}

impl Default for IKSolver {
    fn default() -> Self {
        Self::new(10, 0.001)
    }
}
