use std::iter::once;
use std::ops::RangeInclusive;

use bevy_ecs::prelude::Component;
use indexmap::IndexSet;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::world::lifecycle::coverage::Coverage;

/// The rings of columns past a ticketed column that load with it. A view's ticket stands at the
/// entity-ticking level, and the reference loads a column down to the full level: two steps.
pub const LOADING_RINGS: i32 = 2;

/// The loading tickets a dimension's views hold on its columns, the columns they load, and the
/// sections that still have to spawn or go.
///
/// A ticket loads every section of the columns within [`LOADING_RINGS`] of its own, because a
/// client is sent whole columns. A column that becomes loaded or stops being loaded queues its
/// sections at once, so no system has to propagate the tickets.
#[derive(Component, Debug)]
pub struct ColumnLoading {
    /// Equal tickets are counted where the reference merges them: every view raises its own, so
    /// one view letting go must not take a ticket another view still holds.
    tickets: Coverage<ColumnPos>,
    /// The columns that some ticket loads.
    pub(crate) covered: Coverage<ColumnPos>,
    /// The section rows of the dimension. The simulation clips its areas to them as well.
    pub(crate) section_y: RangeInclusive<i32>,
    /// Became loaded, and `spawn_chunks` has not dealt with it yet.
    pub(crate) pending_spawn: IndexSet<SectionPos, FxBuildHasher>,
    /// Stopped being loaded, and `release_sections` has not dealt with it yet.
    pub(crate) pending_release: FxHashSet<SectionPos>,
    /// Stopped being loaded this tick. The sections still hold their blocks until
    /// `release_sections` lets them go.
    pub(crate) cooling: Vec<ColumnPos>,
}

impl ColumnLoading {
    /// Loads sections within `section_y`: one past the dimension's top or bottom is never
    /// filled, so it would be spawned and condemned again every tick.
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

    /// Takes back one ticket on `column`. A column without a ticket is left alone.
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

    /// Whether some ticket loads `column`.
    pub fn is_column_loaded(&self, column: ColumnPos) -> bool {
        self.covered.contains(column)
    }

    /// Whether a ticket within one column of `column` loads it and the eight around it, so the
    /// dimension lights it from the blocks of its neighbours. The outer ring of the columns that
    /// a ticket loads is loaded but not lit this way.
    pub fn lights(&self, column: ColumnPos) -> bool {
        column
            .square(LOADING_RINGS - 1)
            .any(|near| self.tickets.contains(near))
    }

    /// The columns that stopped being loaded since the sections were last released. A column
    /// that a ticket loads again in the meantime stays in the list.
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
