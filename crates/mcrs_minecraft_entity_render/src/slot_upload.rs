use bevy::prelude::Resource;

use crate::records::{CullRecord, InputRecord, ScatterEntry, StateRecord, TickRecord};

/// The slots of one page.
pub const PAGE_SLOTS: u32 = 64;
/// The bit of a scatter tick for a slot with no previous record.
pub const SCATTER_FRESH: u32 = 0x8000_0000;

/// The records of consecutive slots from `first`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordRange<R> {
    pub first: u32,
    pub records: Vec<R>,
}

/// The writes of the slots since the last upload.
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct SlotUpload {
    /// The slots that the GPU buffers must hold.
    pub slot_capacity: u32,
    /// The slots below which a slot can be valid.
    pub slot_count: u32,
    pub ticks: Vec<ScatterEntry>,
    pub culls: Vec<RecordRange<CullRecord>>,
    pub states: Vec<RecordRange<StateRecord>>,
    pub inputs: Vec<RecordRange<InputRecord>>,
}

impl SlotUpload {
    /// The bytes that the upload writes.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.ticks.len() * size_of::<ScatterEntry>()
            + self
                .culls
                .iter()
                .map(|range| range.records.len())
                .sum::<usize>()
                * size_of::<CullRecord>()
            + self
                .states
                .iter()
                .map(|range| range.records.len())
                .sum::<usize>()
                * size_of::<StateRecord>()
            + self
                .inputs
                .iter()
                .map(|range| range.records.len())
                .sum::<usize>()
                * size_of::<InputRecord>()
    }
}

/// A set of dirty pages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DirtyPages(Vec<u64>);

impl DirtyPages {
    fn mark(&mut self, page: u32) {
        let word = (page / 64) as usize;
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        if let Some(bits) = self.0.get_mut(word) {
            *bits |= 1 << (page % 64);
        }
    }

    fn take_ranges(&mut self) -> Vec<(u32, u32)> {
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for (word, bits) in self.0.iter_mut().enumerate() {
            let mut rest = *bits;
            while rest != 0 {
                let bit = rest.trailing_zeros();
                rest &= rest - 1;
                let page = word as u32 * 64 + bit;
                match ranges.last_mut() {
                    Some((_, end)) if *end == page => *end = page + 1,
                    _ => ranges.push((page, page + 1)),
                }
            }
            *bits = 0;
        }
        ranges
    }
}

/// The CPU side of the slots: the paged arena, a mirror of each record, and the writes that the
/// next upload sends.
#[derive(Debug, Default, Resource)]
pub struct SlotArena {
    validity: Vec<u64>,
    free: Vec<u32>,
    ticks: Vec<TickRecord>,
    culls: Vec<CullRecord>,
    states: Vec<StateRecord>,
    inputs: Vec<InputRecord>,
    pending_ticks: Vec<ScatterEntry>,
    pending_index: Vec<u32>,
    dirty_culls: DirtyPages,
    dirty_states: DirtyPages,
    dirty_inputs: DirtyPages,
    live: u32,
}

const NOT_PENDING: u32 = u32::MAX;

impl SlotArena {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The slots in use.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.live
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The slots of all pages.
    #[must_use]
    pub fn capacity(&self) -> u32 {
        self.validity.len() as u32 * PAGE_SLOTS
    }

    /// A free slot. Its records are zero until the caller writes them.
    pub fn allocate(&mut self) -> u32 {
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                let page = self.validity.len() as u32;
                self.validity.push(0);
                let end = (page + 1) * PAGE_SLOTS;
                self.ticks.resize(end as usize, TickRecord::default());
                self.culls.resize(end as usize, CullRecord::default());
                self.states.resize(end as usize, StateRecord::default());
                self.inputs.resize(end as usize, InputRecord::default());
                self.pending_index.resize(end as usize, NOT_PENDING);
                self.free.extend((page * PAGE_SLOTS + 1..end).rev());
                page * PAGE_SLOTS
            }
        };
        if let Some(bits) = self.validity.get_mut((slot / PAGE_SLOTS) as usize) {
            *bits |= 1 << (slot % PAGE_SLOTS);
        }
        self.live += 1;
        slot
    }

    /// Frees a slot: the cull drops it from the next frame.
    pub fn release(&mut self, slot: u32) {
        let Some(bits) = self.validity.get_mut((slot / PAGE_SLOTS) as usize) else {
            return;
        };
        if *bits & (1 << (slot % PAGE_SLOTS)) == 0 {
            return;
        }
        *bits &= !(1 << (slot % PAGE_SLOTS));
        self.live -= 1;
        self.write_cull(slot, CullRecord::default());
        self.free.push(slot);
    }

    /// Writes the tick record of a slot at the client tick `tick` when it changed. A `fresh`
    /// record has no previous record: the GPU takes it for both.
    pub fn write_tick(&mut self, slot: u32, tick: u32, record: TickRecord, fresh: bool) -> bool {
        let Some(held) = self.ticks.get_mut(slot as usize) else {
            return false;
        };
        if *held == record && !fresh {
            return false;
        }
        *held = record;
        let tick = if fresh {
            tick | SCATTER_FRESH
        } else {
            tick & !SCATTER_FRESH
        };
        let entry = ScatterEntry { slot, tick, record };
        match self.pending_index.get_mut(slot as usize) {
            Some(index) if *index != NOT_PENDING => {
                if let Some(pending) = self.pending_ticks.get_mut(*index as usize) {
                    let fresh = pending.tick & SCATTER_FRESH;
                    *pending = ScatterEntry {
                        tick: entry.tick | fresh,
                        ..entry
                    };
                }
            }
            Some(index) => {
                *index = self.pending_ticks.len() as u32;
                self.pending_ticks.push(entry);
            }
            None => return false,
        }
        true
    }

    #[must_use]
    pub fn cull(&self, slot: u32) -> Option<&CullRecord> {
        self.culls.get(slot as usize)
    }

    #[must_use]
    pub fn state(&self, slot: u32) -> Option<&StateRecord> {
        self.states.get(slot as usize)
    }

    #[must_use]
    pub fn input(&self, slot: u32) -> Option<&InputRecord> {
        self.inputs.get(slot as usize)
    }

    pub fn write_cull(&mut self, slot: u32, record: CullRecord) -> bool {
        write(&mut self.culls, &mut self.dirty_culls, slot, record)
    }

    pub fn write_state(&mut self, slot: u32, record: StateRecord) -> bool {
        write(&mut self.states, &mut self.dirty_states, slot, record)
    }

    pub fn write_input(&mut self, slot: u32, record: InputRecord) -> bool {
        write(&mut self.inputs, &mut self.dirty_inputs, slot, record)
    }

    /// Queues every record of the mirror for the next upload, the tick record of each live slot
    /// as fresh at `tick`: the GPU buffers that a new library builds start empty.
    pub fn resend(&mut self, tick: u32) {
        let live: Vec<u32> = self
            .validity
            .iter()
            .enumerate()
            .flat_map(|(page, bits)| {
                (0..PAGE_SLOTS)
                    .filter(move |bit| bits & (1 << bit) != 0)
                    .map(move |bit| page as u32 * PAGE_SLOTS + bit)
            })
            .collect();
        for page in 0..self.validity.len() as u32 {
            self.dirty_culls.mark(page);
            self.dirty_states.mark(page);
            self.dirty_inputs.mark(page);
        }
        for slot in live {
            if let Some(record) = self.ticks.get(slot as usize).copied() {
                self.write_tick(slot, tick, record, true);
            }
        }
    }

    /// The writes since the last call, and clears them.
    pub fn take_upload(&mut self) -> SlotUpload {
        for entry in &self.pending_ticks {
            if let Some(index) = self.pending_index.get_mut(entry.slot as usize) {
                *index = NOT_PENDING;
            }
        }
        let high = self
            .validity
            .iter()
            .rposition(|bits| *bits != 0)
            .map_or(0, |page| (page as u32 + 1) * PAGE_SLOTS);
        SlotUpload {
            slot_capacity: self.capacity(),
            slot_count: high,
            ticks: std::mem::take(&mut self.pending_ticks),
            culls: ranges(&mut self.dirty_culls, &self.culls),
            states: ranges(&mut self.dirty_states, &self.states),
            inputs: ranges(&mut self.dirty_inputs, &self.inputs),
        }
    }
}

fn write<R: PartialEq>(mirror: &mut [R], dirty: &mut DirtyPages, slot: u32, record: R) -> bool {
    let Some(held) = mirror.get_mut(slot as usize) else {
        return false;
    };
    if *held == record {
        return false;
    }
    *held = record;
    dirty.mark(slot / PAGE_SLOTS);
    true
}

fn ranges<R: Clone>(dirty: &mut DirtyPages, mirror: &[R]) -> Vec<RecordRange<R>> {
    dirty
        .take_ranges()
        .into_iter()
        .filter_map(|(first, end)| {
            let (first, end) = (first * PAGE_SLOTS, end * PAGE_SLOTS);
            let records = mirror.get(first as usize..end as usize)?.to_vec();
            Some(RecordRange { first, records })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_holds_each_changed_record_once_and_the_last_tick_record_of_a_slot() {
        let mut arena = SlotArena::new();
        let a = arena.allocate();
        let b = arena.allocate();
        let record = |x| TickRecord {
            block: [x, 0, 0],
            ..TickRecord::default()
        };
        assert!(arena.write_tick(a, 1, record(1), true));
        assert!(arena.write_tick(a, 2, record(2), false));
        assert!(!arena.write_tick(b, 2, TickRecord::default(), false));
        let state = StateRecord {
            definition: 3,
            ..StateRecord::default()
        };
        assert!(arena.write_state(b, state));
        assert!(!arena.write_state(b, state));
        let upload = arena.take_upload();
        assert_eq!(upload.ticks.len(), 1);
        assert_eq!(upload.ticks[0].record, record(2));
        assert_eq!(
            upload.ticks[0].tick,
            2 | SCATTER_FRESH,
            "the slot stays fresh"
        );
        assert_eq!(upload.states.len(), 1);
        assert_eq!(upload.states[0].records[b as usize], state);
        assert!(arena.take_upload().ticks.is_empty());
    }

    #[test]
    fn a_resend_queues_the_whole_mirror_and_the_live_tick_records_as_fresh() {
        let mut arena = SlotArena::new();
        let a = arena.allocate();
        let b = arena.allocate();
        let record = TickRecord {
            block: [7, 0, 0],
            ..TickRecord::default()
        };
        arena.write_tick(a, 1, record, true);
        arena.release(b);
        arena.take_upload();
        arena.resend(9);
        let upload = arena.take_upload();
        assert_eq!(upload.ticks.len(), 1, "only the live slot");
        assert_eq!(upload.ticks[0].slot, a);
        assert_eq!(upload.ticks[0].record, record);
        assert_eq!(upload.ticks[0].tick, 9 | SCATTER_FRESH);
        for ranges in [
            upload.culls.iter().map(|r| r.records.len()).sum::<usize>(),
            upload.states.iter().map(|r| r.records.len()).sum::<usize>(),
            upload.inputs.iter().map(|r| r.records.len()).sum::<usize>(),
        ] {
            assert_eq!(ranges, arena.capacity() as usize);
        }
    }
}
