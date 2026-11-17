use glam::DVec3;
use mcrs_minecraft_numeric as numeric;

use crate::BodyFlags;
use crate::ControlInput;
use crate::Wanted;
use crate::angles;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveControl {
    operation: Operation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Operation {
    #[default]
    Wait,
    MoveTo(Wanted),
    Jump {
        speed_modifier: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveStep {
    pub forward: f32,
    pub speed: Option<f32>,
    pub yaw: Option<f32>,
    pub jump: bool,
}

const AT_WANTED: f64 = 2.500_000_3e-7_f32 as f64;
const LARGEST_TURN: f32 = 90.0;

impl MoveControl {
    pub fn set_wanted(&mut self, wanted: Wanted) {
        if !matches!(self.operation, Operation::Jump { .. }) {
            self.operation = Operation::MoveTo(wanted);
        }
    }

    pub fn step(&mut self, input: &ControlInput) -> MoveStep {
        let rest = MoveStep {
            forward: 0.0,
            speed: None,
            yaw: None,
            jump: false,
        };
        match self.operation {
            Operation::Wait => rest,
            Operation::Jump { speed_modifier } => {
                let speed = mob_speed(speed_modifier, input.movement_speed);
                let flags = input.flags;
                if flags.contains(BodyFlags::ON_GROUND) || flags.contains(BodyFlags::IN_LIQUID) {
                    self.operation = Operation::Wait;
                }
                MoveStep {
                    forward: speed,
                    speed: Some(speed),
                    ..rest
                }
            }
            Operation::MoveTo(wanted) => {
                self.operation = Operation::Wait;
                self.move_to(input, wanted).unwrap_or(rest)
            }
        }
    }

    fn move_to(&mut self, input: &ControlInput, wanted: Wanted) -> Option<MoveStep> {
        let position: DVec3 = input.position;
        let dx = wanted.position.x - position.x;
        let dy = wanted.position.y - position.y;
        let dz = wanted.position.z - position.z;
        let horizontal = dx * dx + dz * dz;
        if horizontal + dy * dy < AT_WANTED {
            return None;
        }
        let yaw = input.rotations.yaw;
        let turn =
            numeric::wrap_degrees(angles::yaw_to(dx, dz) - yaw).clamp(-LARGEST_TURN, LARGEST_TURN);
        let mut turned = yaw + turn;
        if turned < 0.0 {
            turned += 360.0;
        } else if turned > 360.0 {
            turned -= 360.0;
        }
        let speed = mob_speed(wanted.speed_modifier, input.movement_speed);
        let body = input.body;
        let climbs =
            dy > f64::from(body.step_height) && horizontal < f64::from(body.width.max(1.0));
        let blocked = input.feet_block_top.is_some_and(|top| position.y < top);
        let jump = climbs || blocked;
        if jump {
            self.operation = Operation::Jump {
                speed_modifier: wanted.speed_modifier,
            };
        }
        Some(MoveStep {
            forward: speed,
            speed: Some(speed),
            yaw: Some(turned),
            jump,
        })
    }
}

#[must_use]
pub fn mob_speed(speed_modifier: f32, movement_speed: f64) -> f32 {
    (f64::from(speed_modifier) * movement_speed) as f32
}
