use core::iter;

use mcrs_engine_muxer::EntityId;

use crate::EntityState;
use crate::MotionRow;

/// The rows of the server entities of one tile, one column per field. A spawn or a despawn of an
/// item, an orb or a projectile is a row operation here and never touches ECS; tiles do them in
/// parallel. A despawn moves the last row into the gap, so the row order is not kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileRows {
    state: Vec<EntityState>,
    motion: Vec<MotionRow>,
    id: Vec<EntityId>,
    free_ids: Vec<EntityId>,
}

/// One row, out of its tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub state: EntityState,
    pub motion: MotionRow,
    pub id: EntityId,
}

/// One row in its tile, with its state open to the writer of the phase.
#[derive(Debug, PartialEq)]
pub struct RowMut<'a> {
    pub state: &'a mut EntityState,
    pub motion: &'a MotionRow,
    pub id: EntityId,
}

impl TileRows {
    /// Gives the tile ids from the range of its cell, for the spawns of later ticks.
    pub fn reserve_ids(&mut self, ids: impl IntoIterator<Item = EntityId>) {
        self.free_ids.extend(ids);
    }

    /// Adds a row with one of the reserved ids; `None` when the tile has no reserved id left.
    pub fn spawn_row(&mut self, state: EntityState, motion: MotionRow) -> Option<EntityId> {
        let id = self.free_ids.pop()?;
        self.insert_row(Row { state, motion, id });
        Some(id)
    }

    /// Adds a row that keeps its id, such as a row that comes from another tile.
    pub fn insert_row(&mut self, row: Row) {
        self.state.push(row.state);
        self.motion.push(row.motion);
        self.id.push(row.id);
    }

    /// Removes the row of `id` and gives it; `None` when the tile has no such row.
    pub fn despawn_row(&mut self, id: EntityId) -> Option<Row> {
        let row = self.id.iter().position(|&held| held == id)?;
        Some(Row {
            id: self.id.swap_remove(row),
            motion: self.motion.swap_remove(row),
            state: self.state.swap_remove(row),
        })
    }

    #[must_use]
    pub fn ids(&self) -> &[EntityId] {
        &self.id
    }

    pub fn rows_mut(&mut self) -> impl Iterator<Item = RowMut<'_>> {
        iter::zip(
            iter::zip(&mut self.state, &self.motion),
            self.id.iter().copied(),
        )
        .map(|((state, motion), id)| RowMut { state, motion, id })
    }
}
