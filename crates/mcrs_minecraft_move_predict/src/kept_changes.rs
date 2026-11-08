use std::collections::VecDeque;

use mcrs_minecraft_physics::PlayerState;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use rustc_hash::FxHashMap;

use crate::Change;

/// The changes sent to one player, numbered from 0 in the order of the writes, that a candidate
/// or a recorded tick can still lack.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct KeptChanges {
    first: u64,
    changes: VecDeque<Change>,
}

impl KeptChanges {
    /// The number of the next change.
    pub(crate) fn end(&self) -> u64 {
        self.first.saturating_add(self.changes.len() as u64)
    }

    pub(crate) fn push(&mut self, change: Change) -> u64 {
        let number = self.end();
        self.changes.push_back(change);
        number
    }

    /// Applies the changes from number `from` up to `to`, in order.
    pub(crate) fn apply(&self, state: &mut PlayerState, from: u64, to: u64) {
        for change in self.range(from, to) {
            change.apply(state);
        }
    }

    /// Fills `undone` with the old state of each block that a change from number `from` on sets;
    /// with two changes of one block, the older one wins.
    pub(crate) fn undo_from(&self, from: u64, undone: &mut FxHashMap<BlockPos, BlockStateId>) {
        undone.clear();
        for change in self.range(from, self.end()).rev() {
            if let Change::Block { at, old, .. } = *change {
                undone.insert(at, old);
            }
        }
    }

    /// Drops the changes before number `keep`: no candidate and no record lacks them.
    pub(crate) fn trim(&mut self, keep: u64) {
        while self.first < keep && self.changes.pop_front().is_some() {
            self.first = self.first.saturating_add(1);
        }
    }

    fn range(&self, from: u64, to: u64) -> impl DoubleEndedIterator<Item = &Change> {
        let skip = from.saturating_sub(self.first);
        let take = to.min(self.end()).saturating_sub(from.max(self.first));
        self.changes
            .iter()
            .skip(usize::try_from(skip).unwrap_or(usize::MAX))
            .take(usize::try_from(take).unwrap_or(0))
    }
}
