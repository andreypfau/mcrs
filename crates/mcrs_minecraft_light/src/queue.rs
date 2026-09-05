//! The queue holds no block changes: those are applied to the world
//! immediately and only the resulting [`Influence`] is deferred. Queuing the
//! change itself would mean a batched-out edit had not happened yet.

use std::collections::BTreeSet;

use mcrs_voxel_math::ColumnPos;
use rustc_hash::FxHashMap;

use crate::region::{BlockBox, Influence};

/// Lower is more urgent, like a vanilla ticket level.
pub type Priority = u16;

/// Used when the caller expresses no opinion.
pub const DEFAULT_PRIORITY: Priority = Priority::MAX / 2;

/// Work waiting per section column, ordered by the most urgent priority the
/// column has been given.
///
/// The two stages of the pipeline hold different work — edits that have not
/// reached the world, and influences that have not been lit — but they order it
/// the same way, and "the map and the order agree" is an invariant worth having
/// in one place.
#[derive(Debug)]
pub struct PriorityColumns<T> {
    columns: FxHashMap<ColumnPos, (Priority, T)>,
    /// The same columns again, keyed so that the most urgent sorts first.
    order: BTreeSet<(Priority, ColumnPos)>,
}

impl<T> Default for PriorityColumns<T> {
    fn default() -> Self {
        Self {
            columns: FxHashMap::default(),
            order: BTreeSet::new(),
        }
    }
}

impl<T> PriorityColumns<T> {
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn contains(&self, column: ColumnPos) -> bool {
        self.columns.contains_key(&column)
    }

    pub fn priority_of(&self, column: ColumnPos) -> Option<Priority> {
        self.columns.get(&column).map(|(priority, _)| *priority)
    }

    pub fn get(&self, column: ColumnPos) -> Option<&T> {
        self.columns.get(&column).map(|(_, work)| work)
    }

    /// The work waiting for a column, created if this is the first. A column
    /// already waiting keeps the more urgent of the two priorities, so a nearby
    /// edit pulls the whole column forward.
    pub fn entry(
        &mut self,
        column: ColumnPos,
        priority: Priority,
        empty: impl FnOnce() -> T,
    ) -> &mut T {
        match self.columns.entry(column) {
            std::collections::hash_map::Entry::Occupied(slot) => {
                let (waiting, work) = slot.into_mut();
                if priority < *waiting {
                    self.order.remove(&(*waiting, column));
                    *waiting = priority;
                    self.order.insert((priority, column));
                }
                work
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                self.order.insert((priority, column));
                &mut slot.insert((priority, empty())).1
            }
        }
    }

    /// Re-orders a column, as vanilla does when a chunk's ticket level changes.
    pub fn set_priority(&mut self, column: ColumnPos, priority: Priority) {
        let Some((waiting, _)) = self.columns.get_mut(&column) else {
            return;
        };
        let previous = std::mem::replace(waiting, priority);
        if previous != priority {
            self.order.remove(&(previous, column));
            self.order.insert((priority, column));
        }
    }

    /// Most urgent first.
    pub fn order(&self) -> impl Iterator<Item = (Priority, ColumnPos)> + '_ {
        self.order.iter().copied()
    }

    pub fn remove(&mut self, column: ColumnPos) -> Option<(Priority, T)> {
        let (priority, work) = self.columns.remove(&column)?;
        self.order.remove(&(priority, column));
        Some((priority, work))
    }
}

#[derive(Debug)]
struct ColumnWork {
    influences: Vec<Influence>,
    bounds: BlockBox,
}

#[derive(Debug, Default)]
pub struct LightQueue(PriorityColumns<ColumnWork>);

/// Columns that may be passed over before a scan gives up looking for one that
/// still fits. Priority order is not spatial order, so a ring around the player
/// arrives interleaved and a few misses in a row mean nothing; a few hundred
/// mean every open batch is full, and walking the rest of a backlog of
/// thousands to confirm it costs more than the columns it would find.
const REJECTIONS_BEFORE_STOP: usize = 256;

impl LightQueue {
    /// Number of columns waiting.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn priority_of(&self, column: ColumnPos) -> Option<Priority> {
        self.0.priority_of(column)
    }

    pub fn push(&mut self, column: ColumnPos, influence: Influence) {
        self.push_with_priority(column, influence, DEFAULT_PRIORITY);
    }

    pub fn push_with_priority(
        &mut self,
        column: ColumnPos,
        influence: Influence,
        priority: Priority,
    ) {
        let bounds = influence.bounds();
        let work = self.0.entry(column, priority, || ColumnWork {
            influences: Vec::new(),
            bounds,
        });
        work.bounds = work.bounds.union(bounds);
        work.influences.push(influence);
    }

    pub fn set_priority(&mut self, column: ColumnPos, priority: Priority) {
        self.0.set_priority(column, priority);
    }

    /// Fills up to `max_batches` non-overlapping batches of the most urgent
    /// work, each staying within `budget_cells` and clear of every `avoid` area.
    ///
    /// The budget is in field cells rather than influences because that is what
    /// an epoch costs: the batch is unioned into one box and both the seeding
    /// and the read-back pass walk every cell of it. Measuring the union also
    /// makes a batch spatially coherent for free — a column far from the ones
    /// already taken blows the budget and is left for the next batch.
    ///
    /// Columns are taken whole: splitting one would mean lighting part of a
    /// column against blocks the rest of the batch is about to change, and the
    /// sky source scan spans the whole column anyway. At least one column is
    /// always taken, so a column larger than the budget still makes progress.
    ///
    /// A column that does not fit is passed over rather than ending the batch,
    /// because priority order is not spatial order: a ring of columns around
    /// the player arrives interleaved, and stopping at the first one that
    /// overshoots would hand out batches of two.
    ///
    /// A column whose field would overlap an `avoid` area is passed over for
    /// the same reason: those areas belong to work already in flight, which
    /// publishes every section of its own field and would clobber this batch's
    /// answer there, or be clobbered by it.
    ///
    /// All the batches are filled in one pass, because the scan is over waiting
    /// columns and repeating it per free epoch slot would make the caller's
    /// cost scale with the machine's core count.
    pub fn drain_batches(
        &mut self,
        budget_cells: u64,
        avoid: &[BlockBox],
        max_batches: usize,
    ) -> Vec<Vec<Influence>> {
        let mut areas: Vec<BlockBox> = Vec::new();
        let mut fields: Vec<BlockBox> = Vec::new();
        let mut chosen: Vec<Vec<(Priority, ColumnPos)>> = Vec::new();

        let mut rejections = 0;
        'column: for (priority, column) in self.0.order() {
            if rejections >= REJECTIONS_BEFORE_STOP {
                break;
            }
            let bounds = self.0.get(column).expect("the order names a column").bounds;
            for index in 0..chosen.len() {
                let grown = areas[index].union(bounds);
                // The one cell of margin `prepare_batch` adds is part of the field.
                let field = grown.expand(1).section_aligned();
                if field.cells() > budget_cells {
                    continue;
                }
                if collides(field, avoid, &fields, index) {
                    continue;
                }
                areas[index] = grown;
                fields[index] = field;
                chosen[index].push((priority, column));
                rejections = 0;
                continue 'column;
            }
            if chosen.len() < max_batches {
                let field = bounds.expand(1).section_aligned();
                if !collides(field, avoid, &fields, chosen.len()) {
                    areas.push(bounds);
                    fields.push(field);
                    chosen.push(vec![(priority, column)]);
                    rejections = 0;
                    continue 'column;
                }
            }
            rejections += 1;
        }

        chosen
            .into_iter()
            .map(|batch| {
                let mut taken = Vec::new();
                for (_, column) in batch {
                    let (_, work) = self.0.remove(column).expect("the order names a column");
                    taken.extend(work.influences);
                }
                taken
            })
            .collect()
    }
}

fn collides(field: BlockBox, avoid: &[BlockBox], fields: &[BlockBox], skip: usize) -> bool {
    avoid.iter().any(|other| other.intersects(field))
        || fields
            .iter()
            .enumerate()
            .any(|(index, other)| index != skip && other.intersects(field))
}
