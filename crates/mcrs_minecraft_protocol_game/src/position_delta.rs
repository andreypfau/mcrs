//! Relative positions of the move packets ([`VecDelta`](crate::clientbound::VecDelta)): each
//! axis in steps of 1/4096 block from a base (TRK-WIRE-001). The server encodes and the client
//! decodes with the same rounding, so the two ends agree on every position.

use glam::DVec3;

/// The steps of one block in a relative move.
pub const STEPS_PER_BLOCK: f64 = 4096.0;

/// The steps from `from` to `to` on each axis, or `None` when an axis moves more than an `i16`
/// holds. Both ends round to the step grid, as [`decode`] does, so the rounding of one delta
/// never carries into the next.
#[must_use]
pub fn encode(from: DVec3, to: DVec3) -> Option<[i16; 3]> {
    let axis = |from: f64, to: f64| {
        let steps = grid(to) - grid(from);
        (steps >= f64::from(i16::MIN) && steps <= f64::from(i16::MAX)).then_some(steps as i16)
    };
    Some([
        axis(from.x, to.x)?,
        axis(from.y, to.y)?,
        axis(from.z, to.z)?,
    ])
}

/// The position that `delta` gives from `base`. An axis with a delta of 0 keeps the axis of
/// `base` exactly.
#[must_use]
pub fn decode(base: DVec3, delta: [i16; 3]) -> DVec3 {
    let axis = |base: f64, delta: i16| {
        if delta == 0 {
            base
        } else {
            (grid(base) + f64::from(delta)) / STEPS_PER_BLOCK
        }
    };
    DVec3::new(
        axis(base.x, delta[0]),
        axis(base.y, delta[1]),
        axis(base.z, delta[2]),
    )
}

/// The step of the grid nearest to `coordinate`. Halfway rounds up, also below zero: `round`
/// would take -2.5 to -3.
fn grid(coordinate: f64) -> f64 {
    (coordinate * STEPS_PER_BLOCK + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_axis_keeps_the_base_and_others_snap_to_the_grid() {
        let base = DVec3::new(0.3, 64.0, -2.0);
        assert_eq!(decode(base, [0, 0, 0]), base);
        let moved = decode(base, [0, 4096, 819]);
        assert_eq!(moved.x, 0.3, "an axis with no delta is not rounded");
        assert_eq!(moved.y, 65.0);
        assert_eq!(moved.z, (-8192.0 + 819.0) / 4096.0);
    }

    #[test]
    fn a_halfway_base_rounds_up_below_zero() {
        let base = DVec3::new(-2.5 / 4096.0, 0.0, 0.0);
        assert_eq!(decode(base, [1, 0, 0]).x, -1.0 / 4096.0);
    }

    #[test]
    fn a_decoded_delta_lands_on_the_grid_step_of_the_target() {
        let base = DVec3::new(0.3, -59.999, -2.0001);
        let cases = [
            DVec3::new(0.3, -59.999, -2.0001),
            DVec3::new(1.234_567, -60.0, -1.5),
            DVec3::new(-7.5, -52.0, 5.5),
            DVec3::new(0.3 - 0.5 / 4096.0, -59.999 + 2.5 / 4096.0, -2.0001),
        ];
        for to in cases {
            let delta = encode(base, to).expect("each case moves less than 8 blocks per axis");
            let decoded = decode(base, delta);
            for (axis, (decoded, to)) in [(decoded.x, to.x), (decoded.y, to.y), (decoded.z, to.z)]
                .into_iter()
                .enumerate()
            {
                if delta[axis] != 0 {
                    assert_eq!(
                        decoded * STEPS_PER_BLOCK,
                        (to * STEPS_PER_BLOCK + 0.5).floor(),
                        "a moved axis decodes to the grid step nearest to the target",
                    );
                } else {
                    assert!(
                        (decoded - to).abs() <= 0.5 / STEPS_PER_BLOCK,
                        "an axis that does not move stays within half a step of the target",
                    );
                }
            }
        }
    }

    #[test]
    fn a_move_past_the_range_of_a_step_does_not_encode() {
        assert_eq!(
            encode(DVec3::ZERO, DVec3::new(8.0, 0.0, 0.0)),
            None,
            "8 blocks are 32768 steps, one more than an i16 holds",
        );
        assert_eq!(
            encode(DVec3::ZERO, DVec3::new(-8.0, 0.0, 0.0)),
            Some([i16::MIN, 0, 0]),
        );
    }
}
