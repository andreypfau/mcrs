use crate::scalar::{GameTick, TickPriority};
use crate::section::{SectionOrder, block_pos, bytes, section_at, section_order, table_bytes};
use crate::sequence::{SubTick, TickKey};
use mcrs_minecraft_core::{BlockPos, LocalPos, SectionPos};
use rustc_hash::FxHashMap;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;

pub trait TickKind: Copy + Eq + Ord + Hash + Debug {}

impl<T: Copy + Eq + Ord + Hash + Debug> TickKind for T {}

pub trait Kinds: Copy + Eq + Debug {
    type Block: TickKind;
    type Fluid: TickKind;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledTick<T> {
    pub pos: BlockPos,
    pub kind: T,
    pub key: TickKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Collected<T> {
    pub pos: BlockPos,
    pub kind: T,
    pub sequence: SubTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queued {
    Accepted,
    Duplicate,
}

#[derive(Debug, thiserror::Error)]
#[error("section {} already has a queue here", .ticks.section)]
pub struct SectionOccupied<T: TickKind> {
    pub ticks: SectionTicks<T>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct QueuedTick<T> {
    due: GameTick,
    priority: TickPriority,
    sequence: SubTick,
    local: LocalPos,
    kind: T,
}

impl<T: TickKind> Ord for QueuedTick<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.key(), self.local.index(), self.kind).cmp(&(
            other.key(),
            other.local.index(),
            other.kind,
        ))
    }
}

impl<T: TickKind> PartialOrd for QueuedTick<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: TickKind> QueuedTick<T> {
    fn key(&self) -> TickKey {
        TickKey {
            due: self.due,
            priority: self.priority,
            sequence: self.sequence,
        }
    }

    fn scheduled(&self, section: SectionPos) -> ScheduledTick<T> {
        ScheduledTick {
            pos: block_pos(section, self.local),
            kind: self.kind,
            key: self.key(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedTick<T> {
    pub pos: BlockPos,
    pub kind: T,
    pub delay: i32,
    pub priority: TickPriority,
    pub sequence: SubTick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeldTick<T> {
    sequence: SubTick,
    kind: T,
    delay: i32,
    local: LocalPos,
    priority: TickPriority,
}

impl<T: TickKind> HeldTick<T> {
    fn new(tick: SavedTick<T>) -> Self {
        Self {
            sequence: tick.sequence,
            kind: tick.kind,
            delay: tick.delay,
            local: LocalPos::from(tick.pos),
            priority: tick.priority,
        }
    }

    fn saved(&self, section: SectionPos) -> SavedTick<T> {
        SavedTick {
            pos: block_pos(section, self.local),
            kind: self.kind,
            delay: self.delay,
            priority: self.priority,
            sequence: self.sequence,
        }
    }

    fn activated(&self, now: GameTick) -> QueuedTick<T> {
        QueuedTick {
            due: GameTick(now.0.saturating_add(i64::from(self.delay))),
            priority: self.priority,
            sequence: self.sequence,
            local: self.local,
            kind: self.kind,
        }
    }
}

const KEPT_CAPACITY: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct SectionQueue<T> {
    heap: BinaryHeap<Reverse<QueuedTick<T>>>,
    pending: FxHashMap<(LocalPos, T), u32>,
    indexed: Option<GameTick>,
}

impl<T: TickKind> SectionQueue<T> {
    fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            pending: FxHashMap::default(),
            indexed: None,
        }
    }

    fn head(&self) -> Option<&QueuedTick<T>> {
        self.heap.peek().map(|Reverse(tick)| tick)
    }

    fn heap_bytes(&self) -> usize {
        bytes::<QueuedTick<T>>(self.heap.capacity()).saturating_add(table_bytes::<(
            (LocalPos, T),
            u32,
        )>(
            self.pending.capacity()
        ))
    }

    fn release(&mut self, local: LocalPos, kind: T) {
        if let Entry::Occupied(mut count) = self.pending.entry((local, kind)) {
            if *count.get() > 1 {
                *count.get_mut() = count.get().saturating_sub(1);
            } else {
                count.remove();
            }
        }
    }
}

#[derive(Debug)]
pub struct SectionTicks<T> {
    section: SectionPos,
    queue: SectionQueue<T>,
    held: Vec<HeldTick<T>>,
}

#[derive(Clone, Copy, Debug)]
pub struct SectionTicksRef<'a, T> {
    section: SectionPos,
    queue: &'a SectionQueue<T>,
    held: &'a [HeldTick<T>],
}

impl<T> SectionTicks<T> {
    pub fn as_ref(&self) -> SectionTicksRef<'_, T> {
        SectionTicksRef {
            section: self.section,
            queue: &self.queue,
            held: &self.held,
        }
    }
}

impl<'a, T: TickKind> SectionTicksRef<'a, T> {
    pub fn section(&self) -> SectionPos {
        self.section
    }

    pub fn active(&self) -> impl Iterator<Item = ScheduledTick<T>> + 'a {
        let section = self.section;
        let mut ticks: Vec<QueuedTick<T>> =
            self.queue.heap.iter().map(|Reverse(tick)| *tick).collect();
        ticks.sort_unstable_by_key(|tick| (tick.sequence, tick.local.index(), tick.kind));
        ticks.into_iter().map(move |tick| tick.scheduled(section))
    }

    pub fn held(&self) -> impl Iterator<Item = SavedTick<T>> + 'a {
        let section = self.section;
        self.held.iter().map(move |held| held.saved(section))
    }

    pub fn len(&self) -> usize {
        self.queue.heap.len().saturating_add(self.held.len())
    }

    pub fn is_empty(&self) -> bool {
        self.queue.heap.is_empty() && self.held.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
struct DueIndex {
    heap: BinaryHeap<Reverse<(GameTick, SectionOrder)>>,
    stale: usize,
}

impl DueIndex {
    fn reindex<T: TickKind>(&mut self, section: SectionPos, entry: &mut SectionQueue<T>) {
        let due = entry.head().map(|head| head.due);
        if due == entry.indexed {
            return;
        }
        self.forget(entry);
        if let Some(due) = due {
            // chisle: every head change leaves one stale entry behind; the compaction in
            // `tidy_index` bounds them at max(sections, 64).
            self.heap.push(Reverse((due, section_order(section))));
        }
        entry.indexed = due;
    }

    fn forget<T>(&mut self, entry: &mut SectionQueue<T>) {
        if entry.indexed.take().is_some() {
            self.stale = self.stale.saturating_add(1);
        }
    }
}

#[derive(Clone, Debug)]
pub struct TickQueue<T> {
    sections: FxHashMap<SectionPos, SectionQueue<T>>,
    held: FxHashMap<SectionPos, Vec<HeldTick<T>>>,
    index: DueIndex,
    len: usize,
}

impl<T: TickKind> Default for TickQueue<T> {
    fn default() -> Self {
        Self {
            sections: FxHashMap::default(),
            held: FxHashMap::default(),
            index: DueIndex::default(),
            len: 0,
        }
    }
}

const MIN_STALE_BEFORE_COMPACTION: usize = 64;

impl<T: TickKind> TickQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, tick: ScheduledTick<T>) -> Queued {
        let section = SectionPos::from(tick.pos);
        let local = LocalPos::from(tick.pos);
        let entry = self
            .sections
            .entry(section)
            .or_insert_with(SectionQueue::new);
        match entry.pending.entry((local, tick.kind)) {
            Entry::Occupied(_) => return Queued::Duplicate,
            Entry::Vacant(vacant) => {
                vacant.insert(1);
            }
        }
        entry.heap.push(Reverse(QueuedTick {
            due: tick.key.due,
            priority: tick.key.priority,
            sequence: tick.key.sequence,
            local,
            kind: tick.kind,
        }));
        self.index.reindex(section, entry);
        self.len = self.len.saturating_add(1);
        self.tidy_index();
        Queued::Accepted
    }

    pub fn contains(&self, pos: BlockPos, kind: T) -> bool {
        self.sections
            .get(&SectionPos::from(pos))
            .is_some_and(|entry| entry.pending.contains_key(&(LocalPos::from(pos), kind)))
    }

    pub fn next_due(&self) -> Option<GameTick> {
        self.index.heap.peek().map(|Reverse((due, _))| *due)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn holds(&self, section: SectionPos) -> bool {
        self.held.contains_key(&section)
            || self
                .sections
                .get(&section)
                .is_some_and(|entry| !entry.heap.is_empty())
    }

    pub fn sections(&self) -> impl Iterator<Item = SectionPos> + '_ {
        self.sections.keys().copied()
    }

    pub fn heap_bytes(&self) -> usize {
        self.sections
            .values()
            .map(SectionQueue::heap_bytes)
            .chain(
                self.held
                    .values()
                    .map(|held| bytes::<HeldTick<T>>(held.capacity())),
            )
            .chain([
                table_bytes::<(SectionPos, SectionQueue<T>)>(self.sections.capacity()),
                table_bytes::<(SectionPos, Vec<HeldTick<T>>)>(self.held.capacity()),
                bytes::<(GameTick, SectionOrder)>(self.index.heap.capacity()),
            ])
            .fold(0, usize::saturating_add)
    }

    pub fn due_sections<E>(&mut self, now: GameTick, mut eligible: E, out: &mut Vec<SectionPos>)
    where
        E: FnMut(SectionPos) -> bool,
    {
        out.clear();
        while let Some(&Reverse((due, order))) = self.index.heap.peek() {
            if due > now {
                break;
            }
            self.index.heap.pop();
            let section = section_at(order);
            match self.sections.get_mut(&section) {
                Some(entry) if entry.indexed == Some(due) => entry.indexed = None,
                _ => {
                    self.index.stale = self.index.stale.saturating_sub(1);
                    continue;
                }
            }
            out.push(section);
        }
        for section in out.iter() {
            if let Some(entry) = self.sections.get_mut(section) {
                self.index.reindex(*section, entry);
            }
        }
        // chisle: a section that does not run ticks but has a due head is popped and
        // re-indexed every tick; a wake-up on its property change would lift this.
        out.retain(|section| eligible(*section));
        self.tidy_index();
    }

    pub fn head(&self, section: SectionPos) -> Option<TickKey> {
        self.sections
            .get(&section)
            .and_then(SectionQueue::head)
            .map(QueuedTick::key)
    }

    pub fn collect_head(&mut self, section: SectionPos) -> Option<Collected<T>> {
        let entry = self.sections.get_mut(&section)?;
        let Reverse(tick) = entry.heap.pop()?;
        entry.release(tick.local, tick.kind);
        if entry.heap.is_empty() {
            // chisle: an emptied section keeps up to 64 slots; the number is not measured.
            entry.heap.shrink_to(KEPT_CAPACITY);
            entry.pending.shrink_to(KEPT_CAPACITY);
        }
        self.index.reindex(section, entry);
        self.len = self.len.saturating_sub(1);
        self.tidy_index();
        Some(Collected {
            pos: block_pos(section, tick.local),
            kind: tick.kind,
            sequence: tick.sequence,
        })
    }

    pub fn section_ticks(&self, section: SectionPos) -> Option<SectionTicksRef<'_, T>> {
        self.sections.get(&section).map(|queue| SectionTicksRef {
            section,
            queue,
            held: self.held.get(&section).map_or(&[], Vec::as_slice),
        })
    }

    pub fn take_section(&mut self, section: SectionPos) -> Option<SectionTicks<T>> {
        let mut queue = self.sections.remove(&section)?;
        let held = self.held.remove(&section).unwrap_or_default();
        self.index.forget(&mut queue);
        let ticks = SectionTicks {
            section,
            queue,
            held,
        };
        self.len = self.len.saturating_sub(ticks.as_ref().len());
        self.tidy_index();
        Some(ticks)
    }

    pub fn put_section(&mut self, ticks: SectionTicks<T>) -> Result<(), SectionOccupied<T>> {
        if self.sections.contains_key(&ticks.section) {
            return Err(SectionOccupied { ticks });
        }
        self.len = self.len.saturating_add(ticks.as_ref().len());
        let SectionTicks {
            section,
            mut queue,
            held,
        } = ticks;
        self.index.reindex(section, &mut queue);
        self.sections.insert(section, queue);
        if !held.is_empty() {
            self.held.insert(section, held);
        }
        self.tidy_index();
        Ok(())
    }

    pub fn release_empty(&mut self, section: SectionPos) -> bool {
        !self.holds(section) && self.sections.remove(&section).is_some()
    }

    pub fn hold_saved(&mut self, tick: SavedTick<T>) {
        let local = LocalPos::from(tick.pos);
        let section = SectionPos::from(tick.pos);
        self.sections
            .entry(section)
            .or_insert_with(SectionQueue::new)
            .pending
            .entry((local, tick.kind))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        self.held
            .entry(section)
            .or_default()
            .push(HeldTick::new(tick));
        self.len = self.len.saturating_add(1);
    }

    pub fn activate(&mut self, section: SectionPos, now: GameTick) {
        let Some(held) = self.held.remove(&section) else {
            return;
        };
        let entry = self
            .sections
            .entry(section)
            .or_insert_with(SectionQueue::new);
        entry
            .heap
            .extend(held.iter().map(|held| Reverse(held.activated(now))));
        self.index.reindex(section, entry);
        self.tidy_index();
    }

    fn is_live(&self, due: GameTick, order: SectionOrder) -> bool {
        self.sections
            .get(&section_at(order))
            .is_some_and(|entry| entry.indexed == Some(due))
    }

    fn tidy_index(&mut self) {
        while let Some(&Reverse((due, order))) = self.index.heap.peek() {
            if self.is_live(due, order) {
                break;
            }
            self.index.heap.pop();
            self.index.stale = self.index.stale.saturating_sub(1);
        }
        if self.index.stale > self.sections.len().max(MIN_STALE_BEFORE_COMPACTION) {
            self.index.heap.clear();
            self.index.stale = 0;
            for (section, entry) in &self.sections {
                if let Some(due) = entry.indexed {
                    self.index
                        .heap
                        .push(Reverse((due, section_order(*section))));
                }
            }
        }
    }
}
