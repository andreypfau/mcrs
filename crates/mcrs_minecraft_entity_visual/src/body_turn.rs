use glam::DVec2;
use mcrs_minecraft_numeric as numeric;

use crate::angle;

/// A mob that moves more than this squared horizontal distance in a tick faces its yaw.
pub const MOVING_DISTANCE_SQUARED: f64 = 2.5e-7;
/// A head turn of more than this many degrees since the last turn makes the body follow.
pub const HEAD_TURN_DEGREES: f32 = 15.0;
/// The ticks after a turn in which a still body keeps its yaw.
pub const BODY_REST_TICKS: u32 = 10;
/// The ticks over which a still body then closes the gap to the head.
pub const BODY_CATCH_UP_TICKS: u32 = 10;

/// A living entity that is not a mob, moving faster than this squared distance a tick, turns
/// its body towards its movement.
const WALKER_DISTANCE_SQUARED: f64 = 0.0025;
const WALKER_BACKWARD_MIN: f32 = 95.0;
const WALKER_BACKWARD_MAX: f32 = 265.0;
const WALKER_FOLLOW: f32 = 0.3;
const WALKER_BODY_LIMIT: f32 = 50.0;

/// The yaws that a body turn sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    pub body_yaw: f32,
    pub head_yaw: f32,
}

/// What a mob remembers of its last turn: the head yaw after it and the still ticks since.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyTurn {
    turn_head_yaw: f32,
    ticks_since_turn: u32,
}

impl BodyTurn {
    /// The body yaw of a mob after one tick in which it moved by `movement` (X and Z) and
    /// looks with `head_yaw`. The head stays within `head_limit` of a body that faces `yaw`.
    pub fn follow(
        &mut self,
        yaw: f32,
        head_yaw: f32,
        body_yaw: f32,
        movement: DVec2,
        head_limit: f32,
    ) -> Turn {
        let turn = if movement.length_squared() > MOVING_DISTANCE_SQUARED {
            Turn {
                body_yaw: yaw,
                head_yaw: angle::stay_within(head_yaw, yaw, head_limit),
            }
        } else if (head_yaw - self.turn_head_yaw).abs() > HEAD_TURN_DEGREES {
            Turn {
                body_yaw: angle::stay_within(body_yaw, head_yaw, head_limit),
                head_yaw,
            }
        } else {
            self.ticks_since_turn = self.ticks_since_turn.saturating_add(1);
            let body_yaw = match self.ticks_since_turn.checked_sub(BODY_REST_TICKS) {
                Some(late) if late > 0 => {
                    let closed = (late as f32 / BODY_CATCH_UP_TICKS as f32).min(1.0);
                    angle::stay_within(body_yaw, head_yaw, head_limit * (1.0 - closed))
                }
                _ => body_yaw,
            };
            return Turn { body_yaw, head_yaw };
        };
        self.turn_head_yaw = turn.head_yaw;
        self.ticks_since_turn = 0;
        turn
    }
}

/// The body yaw of a living entity that is not a mob after one tick in which it moved by
/// `movement` (X and Z) and looks with `yaw`.
#[must_use]
pub fn follow_movement(yaw: f32, body_yaw: f32, movement: DVec2) -> f32 {
    let mut target = body_yaw;
    if movement.length_squared() > WALKER_DISTANCE_SQUARED {
        let heading = (movement.y.atan2(movement.x) as f32).to_degrees() - 90.0;
        let off = (numeric::wrap_degrees(yaw) - heading).abs();
        target = if off > WALKER_BACKWARD_MIN && off < WALKER_BACKWARD_MAX {
            heading - 180.0
        } else {
            heading
        };
    }
    let mut body_yaw = body_yaw + WALKER_FOLLOW * numeric::wrap_degrees(target - body_yaw);
    let head = numeric::wrap_degrees(yaw - body_yaw);
    if head.abs() > WALKER_BODY_LIMIT {
        body_yaw += head - WALKER_BODY_LIMIT.copysign(head);
    }
    body_yaw
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: f32 = 75.0;

    #[test]
    fn a_moving_mob_faces_its_yaw_and_holds_the_head_within_the_limit() {
        let mut body = BodyTurn::default();
        let turn = body.follow(10.0, 100.0, -40.0, DVec2::new(0.0, 0.001), LIMIT);
        assert_eq!(
            turn,
            Turn {
                body_yaw: 10.0,
                head_yaw: 85.0
            }
        );
    }

    #[test]
    fn a_still_movement_below_the_threshold_does_not_count() {
        let mut body = BodyTurn::default();
        let still = DVec2::new(0.0, 0.0004);
        assert!(still.length_squared() < MOVING_DISTANCE_SQUARED);
        let turn = body.follow(90.0, 0.0, 0.0, still, LIMIT);
        assert_eq!(turn.body_yaw, 0.0);
    }

    #[test]
    fn a_large_head_turn_drags_the_body_to_the_limit() {
        let mut body = BodyTurn::default();
        let turn = body.follow(0.0, 100.0, 0.0, DVec2::ZERO, LIMIT);
        assert_eq!(turn.body_yaw, 25.0);
    }

    #[test]
    fn a_still_body_rests_ten_ticks_then_reaches_the_head_at_tick_twenty() {
        let mut body = BodyTurn::default();
        let head = 60.0;
        let first = body.follow(0.0, head, 0.0, DVec2::ZERO, LIMIT);
        assert_eq!(
            first.body_yaw, 0.0,
            "the head turn of 60 is within the limit"
        );
        let mut body_yaw = first.body_yaw;
        let mut seen = Vec::new();
        for _ in 1..=20 {
            body_yaw = body
                .follow(0.0, head, body_yaw, DVec2::ZERO, LIMIT)
                .body_yaw;
            seen.push(body_yaw);
        }
        assert!(seen[..10].iter().all(|&yaw| yaw == 0.0), "{seen:?}");
        // Tick 11 allows 67.5, tick 13 allows 52.5: the first that pulls the body.
        assert_eq!(seen[11], 0.0);
        assert!((seen[12] - 7.5).abs() < 1e-4, "{}", seen[12]);
        assert!((seen[16] - 37.5).abs() < 1e-4, "{}", seen[16]);
        assert_eq!(seen[19], head, "the body equals the head from tick 20");
    }

    #[test]
    fn a_walker_turns_towards_its_movement_and_stays_near_its_yaw() {
        let body_yaw = follow_movement(0.0, 0.0, DVec2::new(-0.1, 0.0));
        assert!((body_yaw - 0.3 * 90.0).abs() < 1e-4, "{body_yaw}");
        let backward = follow_movement(0.0, 0.0, DVec2::new(0.0, -0.1));
        assert_eq!(backward, 0.0, "a step backward keeps the body to the yaw");
        let held = follow_movement(0.0, 120.0, DVec2::ZERO);
        assert_eq!(held, 50.0);
    }
}
