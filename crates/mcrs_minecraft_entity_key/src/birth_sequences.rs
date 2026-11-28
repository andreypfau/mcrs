use alloc::collections::BTreeMap;

use crate::BirthSource;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BirthSequences {
    tick: u64,
    next: BTreeMap<[u64; 5], u32>,
}

impl BirthSequences {
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
