//! The queue holds no block changes: those are applied to the world
//! immediately and only the resulting [`Influence`] is deferred. Queuing the
//! change itself would mean a batched-out edit had not happened yet.

use std::collections::BTreeSet;

use mcrs_voxel_math::ColumnPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use rustc_hash::{FxHashMap, FxHashSet};

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

/// Columns off the front of the order to try as batch seeds. A seed already
/// swept up by an earlier batch's growth is skipped, so there have to be more
/// candidates than batches, but never so many that a deep backlog is rescanned.
const SEEDS_PER_BATCH: usize = 32;

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
    /// an epoch costs: every pass of the epoch walks the whole box, so a batch
    /// is worth what its columns bring and costs what its box spans.
    ///
    /// Columns are taken whole: splitting one would mean lighting part of a
    /// column against blocks the rest of the batch is about to change, and the
    /// sky source scan spans the whole column anyway. At least one column is
    /// always taken, so a column larger than the budget still makes progress.
    ///
    /// A batch is seeded from the most urgent column waiting and then grown
    /// outwards from that column, not onwards through the order. Priority is
    /// distance to a player, so the order is a ring: following it drags the box
    /// sideways across the ring and the halo the box carries never amortises
    /// against the columns inside it. Growing outwards keeps the box square, and
    /// a square's halo is a border rather than a second copy of the batch.
    ///
    /// A batch whose field would overlap an `avoid` area is passed over: those
    /// areas belong to work already in flight, which publishes every section of
    /// its own field and would clobber this batch's answer there, or be
    /// clobbered by it.
    pub fn drain_batches(
        &mut self,
        budget_cells: u64,
        avoid: &[BlockBox],
        max_batches: usize,
    ) -> Vec<Vec<Influence>> {
        let mut fields: Vec<BlockBox> = Vec::new();
        let mut chosen: Vec<Vec<ColumnPos>> = Vec::new();
        let mut claimed: FxHashSet<ColumnPos> = FxHashSet::default();

        let seeds: Vec<ColumnPos> = self
            .0
            .order()
            .map(|(_, column)| column)
            .take(max_batches * SEEDS_PER_BATCH)
            .collect();

        for seed in seeds {
            if chosen.len() >= max_batches {
                break;
            }
            if claimed.contains(&seed) {
                continue;
            }
            let Some(work) = self.0.get(seed) else {
                continue;
            };
            let mut area = work.bounds;
            // The one cell of margin `prepare_batch` adds is part of the field.
            let mut field = area.expand(1).section_aligned();
            if collides(field, avoid, &fields) {
                continue;
            }
            let mut batch = vec![seed];
            claimed.insert(seed);

            let mut ring = Vec::new();
            for radius in 1..=reach(field, budget_cells) {
                ring.clear();
                ring.extend(around(seed, radius));
                let mut grew = false;
                for column in ring.drain(..) {
                    if claimed.contains(&column) {
                        continue;
                    }
                    let Some(work) = self.0.get(column) else {
                        continue;
                    };
                    let grown = area.union(work.bounds);
                    let candidate = grown.expand(1).section_aligned();
                    if candidate.cells() > budget_cells || collides(candidate, avoid, &fields) {
                        continue;
                    }
                    area = grown;
                    field = candidate;
                    batch.push(column);
                    claimed.insert(column);
                    grew = true;
                }
                // A ring can be empty because the load front has not reached it,
                // and the one past it be full; two in a row mean this batch has
                // run out of neighbourhood rather than out of luck.
                if !grew && radius > 1 {
                    break;
                }
            }

            fields.push(field);
            chosen.push(batch);
        }

        chosen
            .into_iter()
            .map(|batch| {
                let mut taken = Vec::new();
                for column in batch {
                    let (_, work) = self.0.remove(column).expect("a chosen column is waiting");
                    taken.extend(work.influences);
                }
                taken
            })
            .collect()
    }
}

/// How far out from its seed a batch can reach before the box alone spends the
/// budget, in section columns.
fn reach(seed_field: BlockBox, budget_cells: u64) -> i32 {
    let height = (seed_field.max.y - seed_field.min.y + 1).max(1) as u64;
    let stack = height * (BLOCKS::AREA as u64);
    ((budget_cells / stack.max(1)).isqrt() as i32 / 2).max(1)
}

/// The section columns exactly `radius` steps from `centre`, Chebyshev.
fn around(centre: ColumnPos, radius: i32) -> impl Iterator<Item = ColumnPos> {
    let side = -radius..=radius;
    side.clone()
        .flat_map(move |dx| {
            let side = side.clone();
            side.filter(move |dz| dx.abs() == radius || dz.abs() == radius)
                .map(move |dz| (dx, dz))
        })
        .map(move |(dx, dz)| ColumnPos::new(centre.x + dx, centre.z + dz))
}

fn collides(field: BlockBox, avoid: &[BlockBox], fields: &[BlockBox]) -> bool {
    avoid
        .iter()
        .chain(fields)
        .any(|other| other.intersects(field))
}
