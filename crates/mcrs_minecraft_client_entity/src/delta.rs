//! Relative positions on the wire, in steps of 1/4096 block from a base.

use glam::DVec3;
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};

use crate::path::{PacketPath, PathStep};

const STEPS_PER_BLOCK: f64 = 4096.0;

/// The position that `delta` gives from `base`. An axis with a delta of 0 keeps the axis of
/// `base` exactly.
#[must_use]
pub fn decode(base: DVec3, delta: [i16; 3]) -> DVec3 {
    let axis = |base: f64, delta: i16| {
        if delta == 0 {
            base
        } else {
            // Halfway rounds up, also below zero: `round` would take -2.5 to -3.
            ((base * STEPS_PER_BLOCK + 0.5).floor() + f64::from(delta)) / STEPS_PER_BLOCK
        }
    };
    DVec3::new(
        axis(base.x, delta[0]),
        axis(base.y, delta[1]),
        axis(base.z, delta[2]),
    )
}

/// The path of a move packet. Each step of a stepped delta decodes from the step before it.
#[must_use]
pub fn motion_path(base: DVec3, motion: &VecDelta) -> PacketPath {
    match motion {
        VecDelta::Linear(delta) => PacketPath::Linear(decode(base, *delta)),
        VecDelta::Stepped {
            steps,
            start_immediately,
        } => {
            let mut at = base;
            let steps = steps
                .iter()
                .map(|step| {
                    at = decode(at, step.delta);
                    PathStep {
                        position: at,
                        ticks: tick_count(step.ticks.0),
                    }
                })
                .collect();
            PacketPath::Stepped {
                steps,
                start_immediately: *start_immediately,
            }
        }
    }
}

/// The path of a position sync, which carries absolute positions.
#[must_use]
pub fn sync_path(path: &PositionPath) -> PacketPath {
    match path {
        PositionPath::Linear(position) => PacketPath::Linear(*position),
        PositionPath::Stepped {
            steps,
            start_immediately,
        } => PacketPath::Stepped {
            steps: steps
                .iter()
                .map(|step| PathStep {
                    position: step.position,
                    ticks: tick_count(step.tick_offset.0),
                })
                .collect(),
            start_immediately: *start_immediately,
        },
    }
}

fn tick_count(wire: i32) -> u32 {
    u32::try_from(wire).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol_game::clientbound::DeltaStep;

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
    fn stepped_deltas_chain_from_the_previous_step() {
        let steps = [[0, 0, 4096], [0, 0, 2048]]
            .map(|delta| DeltaStep {
                ticks: VarInt(1),
                delta,
            })
            .to_vec();
        let path = motion_path(
            DVec3::ZERO,
            &VecDelta::Stepped {
                steps,
                start_immediately: false,
            },
        );
        let PacketPath::Stepped { steps, .. } = path else {
            panic!("a stepped delta gives a stepped path");
        };
        let zs: Vec<f64> = steps.iter().map(|step| step.position.z).collect();
        assert_eq!(zs, [1.0, 1.5]);
    }
}
