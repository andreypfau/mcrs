use mcrs_minecraft_numeric as numeric;

use crate::Rotations;
use crate::angles;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyRotation {
    stable_head_yaw: f32,
    stable_count: u32,
}

const MOVED: f64 = 2.500_000_3e-7_f32 as f64;
const STABLE_TURN: f32 = 15.0;
const STABLE_TICKS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobPassenger {
    Yes,
    No,
}

impl BodyRotation {
    pub fn step(
        &mut self,
        dx: f64,
        dz: f64,
        passenger: MobPassenger,
        head_yaw_limit: f32,
        rotations: &mut Rotations,
    ) {
        if dx * dx + dz * dz > MOVED {
            rotations.body_yaw = rotations.yaw;
            rotations.head_yaw =
                angles::limit_to(rotations.head_yaw, rotations.body_yaw, head_yaw_limit);
            self.stable_head_yaw = rotations.head_yaw;
            self.stable_count = 0;
            return;
        }
        if passenger == MobPassenger::Yes {
            return;
        }
        if numeric::wrap_degrees(rotations.head_yaw - self.stable_head_yaw).abs() > STABLE_TURN {
            self.stable_count = 0;
            self.stable_head_yaw = rotations.head_yaw;
            rotations.body_yaw =
                angles::limit_to(rotations.body_yaw, rotations.head_yaw, head_yaw_limit);
            return;
        }
        self.stable_count = self.stable_count.saturating_add(1);
        if self.stable_count > STABLE_TICKS {
            let share = ((self.stable_count - STABLE_TICKS) as f32 / STABLE_TICKS as f32).min(1.0);
            rotations.body_yaw = angles::limit_to(
                rotations.body_yaw,
                rotations.head_yaw,
                head_yaw_limit * (1.0 - share),
            );
        }
    }
}
