use crate::scalar::{GameTick, TickPriority};
use crate::section::{SectionOrder, bytes, section_at, section_order};
use crate::sequence::{SubTick, TickKey};
use crate::tick_queue::{Collected, TickKind, TickQueue};
use mcrs_voxel::SectionPos;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ops::Range;

pub trait OrderingPolicy: Sync {
    fn collect_due<T: TickKind, E>(
        &self,
        queue: &mut TickQueue<T>,
        now: GameTick,
        budget: u32,
        eligible: E,
        scratch: &mut CollectScratch,
        out: &mut Vec<Collected<T>>,
    ) where
        E: FnMut(SectionPos) -> bool,
    {
        out.clear();
        scratch.fill(queue, now, eligible);
        let budget = usize::try_from(budget).unwrap_or(usize::MAX);
        self.order_due(queue, now, budget, scratch, out);
    }

    fn order_due<T: TickKind>(
        &self,
        queue: &mut TickQueue<T>,
        now: GameTick,
        budget: usize,
        scratch: &mut CollectScratch,
        out: &mut Vec<Collected<T>>,
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticOrder;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaOrder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Slot {
    key: TickKey,
    order: SectionOrder,
}

type Candidate = Reverse<(TickPriority, SubTick, usize)>;

#[derive(Clone, Debug, Default)]
pub struct CollectScratch {
    sections: Vec<SectionPos>,
    slots: Vec<Slot>,
    runs: Vec<Range<usize>>,
    candidates: BinaryHeap<Candidate>,
}

impl CollectScratch {
    pub fn heap_bytes(&self) -> usize {
        [
            bytes::<SectionPos>(self.sections.capacity()),
            bytes::<Slot>(self.slots.capacity()),
            bytes::<Range<usize>>(self.runs.capacity()),
            bytes::<Candidate>(self.candidates.capacity()),
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }

    fn fill<T: TickKind, E>(&mut self, queue: &mut TickQueue<T>, now: GameTick, eligible: E)
    where
        E: FnMut(SectionPos) -> bool,
    {
        queue.due_sections(now, eligible, &mut self.sections);
        self.slots.clear();
        self.slots.extend(
            self.sections
                .iter()
                .filter_map(|&section| due_slot(queue, now, section_order(section))),
        );
    }
}

fn due_slot<T: TickKind>(queue: &TickQueue<T>, now: GameTick, order: SectionOrder) -> Option<Slot> {
    queue
        .head(section_at(order))
        .filter(|key| key.due <= now)
        .map(|key| Slot { key, order })
}

fn sift_down(heap: &mut [Slot], mut at: usize) {
    loop {
        let left = at.saturating_mul(2).saturating_add(1);
        let mut best = at;
        for child in [left, left.saturating_add(1)] {
            if let (Some(candidate), Some(current)) = (heap.get(child), heap.get(best))
                && candidate < current
            {
                best = child;
            }
        }
        if best == at {
            return;
        }
        heap.swap(at, best);
        at = best;
    }
}

fn collect_root<T: TickKind>(
    queue: &mut TickQueue<T>,
    now: GameTick,
    run: &mut [Slot],
    out: &mut Vec<Collected<T>>,
) -> usize {
    let Some(root) = run.first().copied() else {
        return 0;
    };
    out.extend(queue.collect_head(section_at(root.order)));
    let mut len = run.len();
    match due_slot(queue, now, root.order) {
        Some(next) => {
            if let Some(slot) = run.first_mut() {
                *slot = next;
            }
        }
        None => {
            len = len.saturating_sub(1);
            run.swap(0, len);
        }
    }
    if let Some(heap) = run.get_mut(..len) {
        sift_down(heap, 0);
    }
    len
}

fn merge_key(slot: &Slot) -> (TickPriority, SubTick) {
    (slot.key.priority, slot.key.sequence)
}

impl OrderingPolicy for SemanticOrder {
    fn order_due<T: TickKind>(
        &self,
        queue: &mut TickQueue<T>,
        now: GameTick,
        budget: usize,
        scratch: &mut CollectScratch,
        out: &mut Vec<Collected<T>>,
    ) {
        scratch.slots.sort_unstable();
        while out.len() < budget && !scratch.slots.is_empty() {
            let len = collect_root(queue, now, &mut scratch.slots, out);
            scratch.slots.truncate(len);
        }
    }
}

impl OrderingPolicy for VanillaOrder {
    fn order_due<T: TickKind>(
        &self,
        queue: &mut TickQueue<T>,
        now: GameTick,
        budget: usize,
        scratch: &mut CollectScratch,
        out: &mut Vec<Collected<T>>,
    ) {
        let CollectScratch {
            slots,
            runs,
            candidates,
            ..
        } = scratch;
        runs.clear();
        candidates.clear();
        let column = |slot: &Slot| (slot.order.0, slot.order.1);
        slots.sort_unstable_by_key(|slot| (column(slot), slot.key, slot.order.2));
        let mut start = 0_usize;
        for chunk in slots.chunk_by(|a, b| column(a) == column(b)) {
            let end = start.saturating_add(chunk.len());
            if let Some(root) = chunk.first() {
                let (priority, sequence) = merge_key(root);
                candidates.push(Reverse((priority, sequence, runs.len())));
            }
            runs.push(start..end);
            start = end;
        }
        while out.len() < budget {
            let Some(Reverse((_, _, index))) = candidates.pop() else {
                break;
            };
            let Some(range) = runs.get_mut(index) else {
                continue;
            };
            let rival = candidates
                .peek()
                .map(|Reverse((priority, sequence, _))| (*priority, *sequence));
            while let Some(run) = slots.get_mut(range.clone()) {
                let len = collect_root(queue, now, run, out);
                range.end = range.start.saturating_add(len);
                let Some(next) = slots.get(range.clone()).and_then(<[Slot]>::first) else {
                    break;
                };
                let key = merge_key(next);
                if out.len() >= budget || rival.is_some_and(|rival| key > rival) {
                    candidates.push(Reverse((key.0, key.1, index)));
                    break;
                }
            }
        }
    }
}
