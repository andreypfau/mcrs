use std::iter::once;
use std::ops::RangeInclusive;

use bevy_ecs::prelude::Component;
use indexmap::IndexSet;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::loading_rings::LOADING_RINGS;
use crate::world::lifecycle::coverage::Coverage;
use crate::world::lifecycle::pending_spawn::PendingSpawn;

#[derive(Component, Debug)]
pub struct ColumnLoading {
    tickets: Coverage<ColumnPos>,
    covered: Coverage<ColumnPos>,
    section_y: RangeInclusive<i32>,
    pending_spawn: IndexSet<SectionPos, FxBuildHasher>,
    pending_release: FxHashSet<SectionPos>,
    cooling: Vec<ColumnPos>,
}

impl ColumnLoading {
    pub fn bounded(section_y: RangeInclusive<i32>) -> Self {
        Self {
            tickets: Coverage::default(),
            covered: Coverage::default(),
            section_y,
            pending_spawn: IndexSet::default(),
            pending_release: FxHashSet::default(),
            cooling: Vec::new(),
        }
    }

    pub fn add_ticket(&mut self, column: ColumnPos) {
        let Self {
            tickets,
            covered,
            section_y,
            pending_spawn,
            pending_release,
            ..
        } = self;
        tickets.raise(once(column), |column| {
            covered.raise(column.square(LOADING_RINGS), |loaded| {
                for pos in sections_of(loaded, section_y) {
                    pending_release.remove(&pos);
                    pending_spawn.insert(pos);
                }
            });
        });
    }

    pub fn remove_ticket(&mut self, column: ColumnPos) {
        let Self {
            tickets,
            covered,
            section_y,
            pending_spawn,
            pending_release,
            cooling,
        } = self;
        tickets.lower(once(column), |column| {
            covered.lower(column.square(LOADING_RINGS), |unloaded| {
                cooling.push(unloaded);
                for pos in sections_of(unloaded, section_y) {
                    pending_spawn.swap_remove(&pos);
                    pending_release.insert(pos);
                }
            });
        });
    }

    #[cfg(test)]
    pub(crate) fn has_ticket(&self, column: ColumnPos) -> bool {
        self.tickets.contains(column)
    }

    pub fn is_column_loaded(&self, column: ColumnPos) -> bool {
        self.covered.contains(column)
    }

    pub fn loaded_columns(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.covered.iter()
    }

    pub fn lights(&self, column: ColumnPos) -> bool {
        column
            .square(LOADING_RINGS - 1)
            .any(|near| self.tickets.contains(near))
    }

    pub fn cooling(&self) -> &[ColumnPos] {
        &self.cooling
    }

    pub fn is_loaded(&self, pos: SectionPos) -> bool {
        is_loaded(&self.covered, &self.section_y, pos)
    }

    #[must_use]
    pub fn section_y(&self) -> &RangeInclusive<i32> {
        &self.section_y
    }

    pub fn requeue(&mut self, pos: SectionPos) {
        self.pending_spawn.insert(pos);
    }

    #[must_use]
    pub fn has_pending_spawns(&self) -> bool {
        !self.pending_spawn.is_empty()
    }

    pub fn take_nearest(
        &mut self,
        centers: &[SectionPos],
        limit: usize,
        taken: &mut Vec<PendingSpawn>,
    ) {
        taken.clear();
        taken.extend(
            self.pending_spawn
                .iter()
                .map(|&pos| PendingSpawn::nearest(pos, centers)),
        );
        if limit < taken.len() {
            taken.select_nth_unstable_by_key(limit, PendingSpawn::distance_sq);
            taken.truncate(limit);
        }
        taken.sort_unstable_by_key(PendingSpawn::distance_sq);
        for pending in taken.iter() {
            self.pending_spawn.swap_remove(&pending.pos());
        }
    }

    pub fn cool_and_release(&mut self, mut keep: impl FnMut(SectionPos) -> bool) {
        self.cooling.clear();
        let Self {
            covered,
            section_y,
            pending_release,
            ..
        } = self;
        pending_release.retain(|&pos| !is_loaded(covered, section_y, pos) && keep(pos));
    }
}

fn is_loaded(
    covered: &Coverage<ColumnPos>,
    section_y: &RangeInclusive<i32>,
    pos: SectionPos,
) -> bool {
    section_y.contains(&pos.y) && covered.contains(ColumnPos::new(pos.x, pos.z))
}

fn sections_of(
    column: ColumnPos,
    section_y: &RangeInclusive<i32>,
) -> impl Iterator<Item = SectionPos> + use<> {
    section_y
        .clone()
        .map(move |y| SectionPos::new(column.x, y, column.z))
}
