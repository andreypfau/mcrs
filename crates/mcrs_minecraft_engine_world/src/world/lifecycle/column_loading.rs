use std::iter::once;
use std::ops::RangeInclusive;

use bevy_ecs::prelude::Component;
use indexmap::IndexSet;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::loading_rings::LOADING_RINGS;
use crate::world::lifecycle::coverage::Coverage;

#[derive(Component, Debug)]
pub struct ColumnLoading {
    tickets: Coverage<ColumnPos>,
    pub(crate) covered: Coverage<ColumnPos>,
    pub(crate) section_y: RangeInclusive<i32>,
    pub(crate) pending_spawn: IndexSet<SectionPos, FxBuildHasher>,
    pub(crate) pending_release: FxHashSet<SectionPos>,
    pub(crate) cooling: Vec<ColumnPos>,
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
}

pub(crate) fn is_loaded(
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
