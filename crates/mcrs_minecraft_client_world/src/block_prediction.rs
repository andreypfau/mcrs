use std::collections::HashMap;

use bevy::ecs::prelude::Resource;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The block changes the client made before the server answered them, and the sequence counter
/// of the world they were made in.
#[derive(Resource, Default, Debug)]
pub struct BlockPredictions {
    sequence: i32,
    records: HashMap<BlockPos, Record>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record {
    sequence: i32,
    server_state: BlockStateId,
}

/// A record that an acknowledgement settled: the state the server holds at `pos`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settled {
    pub pos: BlockPos,
    pub server_state: BlockStateId,
}

impl BlockPredictions {
    /// The sequence of a new prediction. The counter wraps after `i32::MAX`, as the game's
    /// client does, although a server closes the connection on a negative sequence.
    pub fn next_sequence(&mut self) -> i32 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    /// Notes that the prediction of `sequence` changed the block at `pos` away from `old`. A
    /// position that already has a record keeps the server state of its first change.
    pub fn record(&mut self, sequence: i32, pos: BlockPos, old: BlockStateId) {
        self.records
            .entry(pos)
            .and_modify(|record| record.sequence = sequence)
            .or_insert(Record {
                sequence,
                server_state: old,
            });
    }

    /// Takes a state the server sent for `pos`. Answers the state to write into the world, or
    /// `None` when a prediction holds the position until its acknowledgement.
    pub fn server_update(&mut self, pos: BlockPos, state: BlockStateId) -> Option<BlockStateId> {
        match self.records.get_mut(&pos) {
            Some(record) => {
                record.server_state = state;
                None
            }
            None => Some(state),
        }
    }

    /// Removes each record with a sequence of `sequence` or less.
    pub fn acknowledge(&mut self, sequence: i32) -> impl Iterator<Item = Settled> + '_ {
        self.records
            .extract_if(move |_, record| record.sequence <= sequence)
            .map(|(pos, record)| Settled {
                pos,
                server_state: record.server_state,
            })
    }

    /// A new world starts its counter at 0 and holds no records.
    pub fn clear(&mut self) {
        self.sequence = 0;
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(number: u16) -> BlockStateId {
        BlockStateId::checked(number, 100).expect("the test table holds 100 states")
    }

    fn settled(predictions: &mut BlockPredictions, sequence: i32) -> Vec<Settled> {
        let mut settled: Vec<Settled> = predictions.acknowledge(sequence).collect();
        settled.sort_by_key(|settled| settled.pos.x);
        settled
    }

    #[test]
    fn cbk_seq_002_the_first_sequence_of_a_world_is_1() {
        let mut predictions = BlockPredictions::default();
        assert_eq!(predictions.next_sequence(), 1);
        assert_eq!(predictions.next_sequence(), 2);
        predictions.clear();
        assert_eq!(predictions.next_sequence(), 1);
    }

    #[test]
    fn cbk_prd_007_a_server_state_for_a_recorded_position_waits_for_the_acknowledgement() {
        let mut predictions = BlockPredictions::default();
        let (broken, other) = (BlockPos::new(0, 0, 0), BlockPos::new(1, 0, 0));
        predictions.record(1, broken, state(1));

        assert_eq!(predictions.server_update(other, state(2)), Some(state(2)));
        assert_eq!(predictions.server_update(broken, state(3)), None);
        assert_eq!(
            settled(&mut predictions, 1),
            [Settled {
                pos: broken,
                server_state: state(3)
            }]
        );
        assert_eq!(predictions.server_update(broken, state(4)), Some(state(4)));
    }

    #[test]
    fn cbk_prd_008_an_acknowledgement_settles_each_record_up_to_its_sequence() {
        let mut predictions = BlockPredictions::default();
        let (first, second, again) = (
            BlockPos::new(0, 0, 0),
            BlockPos::new(1, 0, 0),
            BlockPos::new(2, 0, 0),
        );
        predictions.record(1, first, state(1));
        predictions.record(2, second, state(2));
        predictions.record(1, again, state(3));
        predictions.record(3, again, state(4));

        assert_eq!(
            settled(&mut predictions, 2),
            [
                Settled {
                    pos: first,
                    server_state: state(1)
                },
                Settled {
                    pos: second,
                    server_state: state(2)
                },
            ]
        );
        assert_eq!(
            settled(&mut predictions, 3),
            [Settled {
                pos: again,
                server_state: state(3)
            }],
            "a later change of a recorded position keeps the state before the first change"
        );
    }
}
