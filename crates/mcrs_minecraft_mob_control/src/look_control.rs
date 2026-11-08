use mcrs_minecraft_brain::LookIntent;
use mcrs_minecraft_brain::Point;

use crate::ControlInput;
use crate::angles;

/// The head turn speed and the pitch limit of a mob type, for the look targets that name none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookDefaults {
    pub turn_speed: f32,
    pub pitch_limit: f32,
}

impl LookDefaults {
    /// The defaults of the animals (GOAL-ANI-006).
    pub const ANIMAL: Self = Self {
        turn_speed: 10.0,
        pitch_limit: 40.0,
    };
}

/// The look control (GOAL-CTL-004) with the look target, and the goal tick of the look around,
/// which sets a look target at the eye height in its direction in each tick (GOAL-LKR-002).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookControl {
    target: Option<LookAt>,
    around: Option<Direction>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LookAt {
    point: Point,
    turn_speed: f32,
    pitch_limit: f32,
    hold: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Direction {
    cos: f32,
    sin: f32,
}

/// The head of a mob after the look control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Head {
    pub head_yaw: f32,
    pub pitch: f32,
}

const NEAR: f64 = 1.0e-5;
const RETURN_SPEED: f32 = 10.0;
const AROUND_HOLD: u8 = 2;

impl LookControl {
    pub fn apply(&mut self, intent: &LookIntent) {
        match *intent {
            LookIntent::At {
                point,
                turn_speed,
                pitch_limit,
                hold,
            } => {
                self.around = None;
                self.target = Some(LookAt {
                    point,
                    turn_speed,
                    pitch_limit,
                    hold,
                });
            }
            LookIntent::Around { cos, sin } => self.around = Some(Direction { cos, sin }),
            LookIntent::EndAround => self.around = None,
        }
    }

    /// The look control of one tick, after the move control. `body_yaw` is the body yaw of the
    /// row; `path_running` limits the head to `head_yaw_limit` from the body.
    pub fn step(
        &mut self,
        input: &ControlInput,
        defaults: &LookDefaults,
        head_yaw: f32,
        path_running: bool,
    ) -> Head {
        let position = input.position;
        let eye = position.y + f64::from(input.body.eye_height);
        if let Some(direction) = self.around {
            self.target = Some(LookAt {
                point: Point::new(
                    position.x + f64::from(direction.cos),
                    eye,
                    position.z + f64::from(direction.sin),
                ),
                turn_speed: defaults.turn_speed,
                pitch_limit: defaults.pitch_limit,
                hold: AROUND_HOLD,
            });
        }
        let body_yaw = input.rotations.body_yaw;
        let mut head = Head {
            head_yaw,
            pitch: 0.0,
        };
        match &mut self.target {
            Some(target) if target.hold > 0 => {
                target.hold -= 1;
                let dx = target.point.x - position.x;
                let dy = target.point.y - eye;
                let dz = target.point.z - position.z;
                let horizontal = (dx * dx + dz * dz).sqrt();
                if dx.abs() > NEAR || dz.abs() > NEAR {
                    head.head_yaw =
                        angles::turn_toward(head_yaw, angles::yaw_to(dx, dz), target.turn_speed);
                }
                if dy.abs() > NEAR || horizontal > NEAR {
                    let limit = target.pitch_limit;
                    head.pitch =
                        angles::wrap(angles::pitch_to(dy, horizontal)).clamp(-limit, limit);
                }
            }
            _ => {
                self.target = None;
                head.head_yaw = angles::turn_toward(head_yaw, body_yaw, RETURN_SPEED);
            }
        }
        if path_running {
            head.head_yaw = angles::limit_to(head.head_yaw, body_yaw, input.body.head_yaw_limit);
        }
        head
    }
}
