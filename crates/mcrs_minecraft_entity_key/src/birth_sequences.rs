use alloc::collections::BTreeMap;

use crate::BirthSource;

/// The next sequence of each birth source in the current tick of one world: the births of one
/// source in one tick number from 0 in the order of the calls.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BirthSequences {
    tick: u64,
    next: BTreeMap<[u64; 5], u32>,
}

impl BirthSequences {
    /// The sequence of the next birth of `source` in `tick`. A new tick starts every source at 0.
    pub fn next(&mut self, tick: u64, source: &BirthSource) -> u32 {
        if self.tick != tick {
            self.tick = tick;
            self.next.clear();
        }
        let [a, b, c, d] = source.words();
        let next = self.next.entry([source.kind(), a, b, c, d]).or_default();
        let sequence = *next;
        *next = next.wrapping_add(1);
        sequence
    }
}
