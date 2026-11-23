use glam::DVec2;
use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;

use crate::Footing;

const MIN_LENGTH_SQUARED: f64 = 1.0E-5;
const JITTER: f64 = 0.01;
const MAX_LIFT: f64 = 0.4;

#[must_use]
pub fn knockback(
    velocity: DVec3,
    direction: DVec2,
    strength: f64,
    resistance: f64,
    footing: Footing,
    stream: &KeyedStream,
) -> DVec3 {
    let power = strength * (1.0 - resistance);
    if power <= 0.0 {
        return velocity;
    }
    let mut direction = direction;
    let mut draw: u64 = 0;
    while direction.length_squared() < MIN_LENGTH_SQUARED {
        let x = (stream.unit(draw) - stream.unit(draw.wrapping_add(1))) * JITTER;
        let z = (stream.unit(draw.wrapping_add(2)) - stream.unit(draw.wrapping_add(3))) * JITTER;
        direction = DVec2::new(x, z);
        draw = draw.wrapping_add(4);
    }
    let push = direction / direction.length() * power;
    let y = match footing {
        Footing::Ground => f64::min(MAX_LIFT, velocity.y / 2.0 + power),
        Footing::Air => velocity.y,
    };
    DVec3::new(velocity.x / 2.0 - push.x, y, velocity.z / 2.0 - push.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_entity_key::EntityUuid;

    const DEFAULT: f64 = 0.4;
    const SOUTH_TO_ATTACKER: DVec2 = DVec2::new(0.0, -2.0);
    const EAST_TO_ATTACKER: DVec2 = DVec2::new(-2.0, 0.0);
    const YAW_ZERO_EXTRA: DVec2 = DVec2::new(0.0, -1.0);

    #[derive(Clone, Copy)]
    struct Push {
        direction: DVec2,
        strength: f64,
    }

    struct Case<'a> {
        pushes: &'a [Push],
        resistance: f64,
        expected: DVec3,
    }

    #[test]
    fn attack_experiments() {
        let default = |direction| Push {
            direction,
            strength: DEFAULT,
        };
        let extra = Push {
            direction: YAW_ZERO_EXTRA,
            strength: 1.0,
        };
        let pig = [default(SOUTH_TO_ATTACKER)];
        let sheep = [default(EAST_TO_ATTACKER), extra];
        let goat = [default(SOUTH_TO_ATTACKER), extra];
        let cases = [
            Case {
                pushes: &pig,
                resistance: 0.0,
                expected: DVec3::new(0.0, 0.4, 0.4),
            },
            Case {
                pushes: &sheep,
                resistance: 0.0,
                expected: DVec3::new(0.2, 0.4, 1.0),
            },
            Case {
                pushes: &goat,
                resistance: 1.0,
                expected: DVec3::ZERO,
            },
        ];
        let stream = KeyedStream::new(&EntityUuid::from_u128(1), 0);
        for case in cases {
            let velocity = case.pushes.iter().fold(DVec3::ZERO, |velocity, push| {
                knockback(
                    velocity,
                    push.direction,
                    push.strength,
                    case.resistance,
                    Footing::Ground,
                    &stream,
                )
            });
            assert!(
                velocity.abs_diff_eq(case.expected, 1.0E-12),
                "{velocity} != {}",
                case.expected
            );
        }
    }
}
