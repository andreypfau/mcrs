use mcrs_voxel::SectionPos;
use std::collections::BTreeSet;

use mcrs_voxel::ColumnPos;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::region::Influence;
use mcrs_minecraft_core::BoundingBox;

pub type Priority = u16;

pub const DEFAULT_PRIORITY: Priority = Priority::MAX / 2;

#[derive(Debug)]
pub struct PriorityColumns<T> {
    columns: FxHashMap<ColumnPos, (Priority, T)>,
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

    pub fn reprioritize(&mut self, mut score: impl FnMut(ColumnPos) -> Priority) {
        self.order.clear();
        for (column, (priority, _)) in self.columns.iter_mut() {
            *priority = score(*column);
            self.order.insert((*priority, *column));
        }
    }

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
    bounds: BoundingBox,
}

#[derive(Debug, Default)]
pub struct LightQueue(PriorityColumns<ColumnWork>);

const SEEDS_PER_BATCH: usize = 32;

impl LightQueue {
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

    pub fn reprioritize(&mut self, score: impl FnMut(ColumnPos) -> Priority) {
        self.0.reprioritize(score);
    }

    pub fn drain_batches(
        &mut self,
        budget_cells: u64,
        avoid: &[BoundingBox],
        max_batches: usize,
    ) -> Vec<Vec<Influence>> {
        let mut fields: Vec<BoundingBox> = Vec::new();
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
            let mut field = area.inflated(1).section_aligned();
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
                    let candidate = grown.inflated(1).section_aligned();
                    if candidate.cells() > budget_cells || collides(candidate, avoid, &fields) {
                        continue;
                    }
                    area = grown;
                    field = candidate;
                    batch.push(column);
                    claimed.insert(column);
                    grew = true;
                }
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
                batch
                    .into_iter()
                    .filter_map(|column| self.0.remove(column))
                    .flat_map(|(_, work)| work.influences)
                    .collect()
            })
            .collect()
    }
}

fn reach(seed_field: BoundingBox, budget_cells: u64) -> i32 {
    let height = (seed_field.max.y - seed_field.min.y + 1).max(1) as u64;
    let stack = height * (SectionPos::AREA as u64);
    ((budget_cells / stack.max(1)).isqrt() as i32 / 2).max(1)
}

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

fn collides(field: BoundingBox, avoid: &[BoundingBox], fields: &[BoundingBox]) -> bool {
    avoid
        .iter()
        .chain(fields)
        .any(|other| other.intersects(field))
}
