//! The paths of the move packets and of the position syncs, as positions from a base.

use glam::DVec3;
use mcrs_minecraft_entity_visual::{PacketPath, PathStep};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::position_delta;

/// The path of a move packet. Each step of a stepped delta decodes from the step before it.
#[must_use]
pub fn motion_path(base: DVec3, motion: &VecDelta) -> PacketPath {
    match motion {
        VecDelta::Linear(delta) => PacketPath::Linear(position_delta::decode(base, *delta)),
        VecDelta::Stepped {
            steps,
            start_immediately,
        } => {
            let mut at = base;
            let steps = steps
                .iter()
                .map(|step| {
                    at = position_delta::decode(at, step.delta);
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
