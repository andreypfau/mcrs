//! The queue holds no block changes: those are applied to the world
//! immediately and only the resulting [`Influence`] is deferred. Queuing the
//! change itself would mean a batched-out edit had not happened yet.

use std::collections::{BTreeSet, HashMap};

use mcrs_voxel_math::ColumnPos;

use crate::region::{BlockBox, Influence};

/// Lower is more urgent, like a vanilla ticket level.
pub type Priority = u16;

/// Used when the caller expresses no opinion.
pub const DEFAULT_PRIORITY: Priority = Priority::MAX / 2;

#[derive(Debug)]
struct ColumnWork {
    influences: Vec<Influence>,
    bounds: BlockBox,
    priority: Priority,
}

#[derive(Debug, Default)]
pub struct LightQueue {
    columns: HashMap<ColumnPos, ColumnWork>,
    /// The same columns again, keyed so that the most urgent sorts first.
    order: BTreeSet<(Priority, ColumnPos)>,
    outstanding: usize,
}

impl LightQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of influences waiting, across all columns.
    pub fn len(&self) -> usize {
        self.outstanding
    }

    pub fn is_empty(&self) -> bool {
        self.outstanding == 0
    }

    pub fn columns_waiting(&self) -> usize {
        self.columns.len()
    }

    pub fn priority_of(&self, column: ColumnPos) -> Option<Priority> {
        self.columns.get(&column).map(|work| work.priority)
    }

    pub fn push(&mut self, column: ColumnPos, influence: Influence) {
        self.push_with_priority(column, influence, DEFAULT_PRIORITY);
    }

    /// Adds work. A column that is already waiting keeps the more urgent of the
    /// two priorities, so a nearby edit pulls the whole column forward.
    pub fn push_with_priority(
        &mut self,
        column: ColumnPos,
        influence: Influence,
        priority: Priority,
    ) {
        self.outstanding += 1;
        match self.columns.get_mut(&column) {
            Some(work) => {
                work.bounds = work.bounds.union(influence.bounds());
                work.influences.push(influence);
                if priority < work.priority {
                    self.order.remove(&(work.priority, column));
                    work.priority = priority;
                    self.order.insert((priority, column));
                }
            }
            None => {
                self.columns.insert(
                    column,
                    ColumnWork {
                        bounds: influence.bounds(),
                        influences: vec![influence],
                        priority,
                    },
                );
                self.order.insert((priority, column));
            }
        }
    }

    /// Re-orders a column, as vanilla does when a chunk's ticket level changes.
    pub fn set_priority(&mut self, column: ColumnPos, priority: Priority) {
        let Some(work) = self.columns.get_mut(&column) else {
            return;
        };
        let previous = std::mem::replace(&mut work.priority, priority);
        if previous != priority {
            self.order.remove(&(previous, column));
            self.order.insert((priority, column));
        }
    }

    /// Takes the most urgent work whose working field stays within
    /// `budget_cells` and does not touch any of the `avoid` areas.
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
    pub fn drain_batch(&mut self, budget_cells: u64, avoid: &[BlockBox]) -> Vec<Influence> {
        self.drain_batches(budget_cells, avoid, 1)
            .pop()
            .unwrap_or_default()
    }

    /// Fills up to `max_batches` batches in one pass over the queue, each one
    /// as [`drain_batch`](Self::drain_batch) would fill it and none of them
    /// overlapping another.
    ///
    /// One pass rather than one per batch: the scan is over every waiting
    /// column, so repeating it per free epoch slot makes the caller's cost
    /// scale with the machine's core count.
    pub fn drain_batches(
        &mut self,
        budget_cells: u64,
        avoid: &[BlockBox],
        max_batches: usize,
    ) -> Vec<Vec<Influence>> {
        let mut areas: Vec<BlockBox> = Vec::new();
        let mut fields: Vec<BlockBox> = Vec::new();
        let mut chosen: Vec<Vec<(Priority, ColumnPos)>> = Vec::new();

        'column: for &(priority, column) in &self.order {
            let bounds = self.columns[&column].bounds;
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
                continue 'column;
            }
            if chosen.len() < max_batches {
                let field = bounds.expand(1).section_aligned();
                if collides(field, avoid, &fields, chosen.len()) {
                    continue;
                }
                areas.push(bounds);
                fields.push(field);
                chosen.push(vec![(priority, column)]);
            }
        }

        chosen
            .into_iter()
            .map(|batch| {
                let mut taken = Vec::new();
                for entry in batch {
                    self.order.remove(&entry);
                    let work = self
                        .columns
                        .remove(&entry.1)
                        .expect("the order names a column");
                    self.outstanding -= work.influences.len();
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
