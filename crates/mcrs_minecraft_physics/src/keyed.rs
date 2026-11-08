use mcrs_engine_muxer::EntityId;

use crate::BirthKey;
use crate::WriterKey;

/// A write or a claim on one entity, with the keys that order it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyed<E> {
    pub target: BirthKey,
    /// Finds the row of the target; the order never reads it.
    pub target_id: EntityId,
    pub writer: WriterKey,
    /// The number of the entry among the entries of one writer in one tick.
    pub sequence: u32,
    pub effect: E,
}
