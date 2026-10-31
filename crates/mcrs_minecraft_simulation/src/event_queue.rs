use crate::section::{SectionOrder, bytes, section_order, table_bytes};
use crate::sequence::SubTick;
use crate::tick_queue::Queued;
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};
use rustc_hash::FxHashSet;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem::take;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockEvent<B> {
    pub pos: BlockPos,
    pub block: B,
    pub a: i32,
    pub b: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueuedEvent<B> {
    pub sequence: SubTick,
    pub event: BlockEvent<B>,
}

impl<B> QueuedEvent<B> {
    fn section(&self) -> SectionPos {
        SectionPos::from(self.event.pos)
    }

    fn order(&self) -> (SubTick, SectionOrder, usize) {
        (
            self.sequence,
            section_order(self.section()),
            LocalPos::from(self.event.pos).index(),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrainEnd {
    pub deferred: u32,
    pub left_by_bound: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drained<B> {
    Event(QueuedEvent<B>),
    Done(DrainEnd),
}

#[derive(Clone, Debug)]
pub struct EventQueue<B> {
    pending: VecDeque<QueuedEvent<B>>,
    undelivered: FxHashSet<BlockEvent<B>>,
    deferred: Vec<QueuedEvent<B>>,
    taken_this_drain: u32,
}

impl<B> Default for EventQueue<B> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            undelivered: FxHashSet::default(),
            deferred: Vec::new(),
            taken_this_drain: 0,
        }
    }
}

impl<B: Copy + Eq + Hash + Debug> EventQueue<B> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue(&mut self, event: QueuedEvent<B>) -> Queued {
        if !self.undelivered.insert(event.event) {
            return Queued::Duplicate;
        }
        let order = event.order();
        let at = self
            .pending
            .partition_point(|queued| queued.order() <= order);
        self.pending.insert(at, event);
        Queued::Accepted
    }

    pub fn next_eligible<E>(&mut self, bound: u32, mut eligible: E) -> Drained<B>
    where
        E: FnMut(SectionPos) -> bool,
    {
        while self.taken_this_drain < bound {
            let Some(event) = self.pending.pop_front() else {
                break;
            };
            if eligible(event.section()) {
                self.taken_this_drain = self.taken_this_drain.saturating_add(1);
                self.undelivered.remove(&event.event);
                return Drained::Event(event);
            }
            let order = event.order();
            let at = self
                .deferred
                .partition_point(|deferred| deferred.order() <= order);
            self.deferred.insert(at, event);
        }
        let end = DrainEnd {
            deferred: u32::try_from(self.deferred.len()).unwrap_or(u32::MAX),
            left_by_bound: u32::try_from(self.pending.len()).unwrap_or(u32::MAX),
        };
        self.taken_this_drain = 0;
        for event in self.deferred.drain(..).rev() {
            let order = event.order();
            let at = self
                .pending
                .partition_point(|queued| queued.order() < order);
            self.pending.insert(at, event);
        }
        Drained::Done(end)
    }

    pub fn absorb(&mut self, other: EventQueue<B>) {
        let deferred = merge(take(&mut self.deferred), other.deferred, QueuedEvent::order);
        let pending = merge(take(&mut self.pending), other.pending, QueuedEvent::order);
        self.undelivered.clear();
        for (was_deferred, event) in merge(
            deferred.map(|event| (true, event)),
            pending.map(|event| (false, event)),
            |(_, event)| event.order(),
        ) {
            if !self.undelivered.insert(event.event) {
                continue;
            }
            if was_deferred {
                self.deferred.push(event);
            } else {
                self.pending.push_back(event);
            }
        }
    }

    pub fn split_off<M>(&mut self, mut moves: M) -> EventQueue<B>
    where
        M: FnMut(SectionPos) -> bool,
    {
        let mut split = EventQueue::new();
        let undelivered = &mut self.undelivered;
        // chisle: scans every pending event, as `take_section` does; a per-section index
        // lifts it if a measurement asks.
        let mut moved = Vec::new();
        let mut keep = |event: &QueuedEvent<B>| {
            if !moves(event.section()) {
                return true;
            }
            undelivered.remove(&event.event);
            split.undelivered.insert(event.event);
            moved.push(*event);
            false
        };
        let deferred = self.deferred.len();
        self.deferred.retain(&mut keep);
        let moved_deferred = deferred.saturating_sub(self.deferred.len());
        self.pending.retain(&mut keep);
        split.pending = moved.drain(moved_deferred.min(moved.len())..).collect();
        split.deferred = moved;
        split
    }

    pub fn take_section(&mut self, section: SectionPos, out: &mut Vec<QueuedEvent<B>>) {
        let start = out.len();
        let undelivered = &mut self.undelivered;
        let mut keep = |event: &QueuedEvent<B>| {
            if event.section() != section {
                return true;
            }
            undelivered.remove(&event.event);
            out.push(*event);
            false
        };
        // chisle: scans every pending event; a per-section index lifts it if unloads
        // during long event queues show up in a measurement.
        self.deferred.retain(&mut keep);
        self.pending.retain(&mut keep);
        if let Some(taken) = out.get_mut(start..) {
            taken.sort_by_key(QueuedEvent::order);
        }
    }

    pub fn len(&self) -> usize {
        self.pending.len().saturating_add(self.deferred.len())
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.deferred.is_empty()
    }

    pub fn heap_bytes(&self) -> usize {
        [
            bytes::<QueuedEvent<B>>(self.pending.capacity()),
            bytes::<QueuedEvent<B>>(self.deferred.capacity()),
            table_bytes::<BlockEvent<B>>(self.undelivered.capacity()),
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }
}

fn merge<T, K, L, R, O>(a: L, b: R, order: O) -> impl Iterator<Item = T>
where
    K: Ord,
    L: IntoIterator<Item = T>,
    R: IntoIterator<Item = T>,
    O: Fn(&T) -> K,
{
    let mut a = a.into_iter().peekable();
    let mut b = b.into_iter().peekable();
    std::iter::from_fn(move || match (a.peek(), b.peek()) {
        (Some(x), Some(y)) if order(y) < order(x) => b.next(),
        (Some(_), _) => a.next(),
        (None, _) => b.next(),
    })
}
