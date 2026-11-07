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

impl TileRows {
    /// Gives the tile ids from the range of its cell, for the spawns of later ticks.
    pub fn reserve_ids(&mut self, ids: impl IntoIterator<Item = EntityId>) {
        self.free_ids.extend(ids);
    }

    /// Adds a row with one of the reserved ids; `None` when the tile has no reserved id left.
    pub fn spawn_row(&mut self, state: EntityState, motion: MotionRow) -> Option<EntityId> {
        let id = self.free_ids.pop()?;
        self.state.push(state);
        self.motion.push(motion);
        self.id.push(id);
        Some(id)
    }

    /// Removes the row of `id` and gives its state; `None` when the tile has no such row.
    pub fn despawn_row(&mut self, id: EntityId) -> Option<EntityState> {
        let row = self.id.iter().position(|&held| held == id)?;
        self.id.swap_remove(row);
        self.motion.swap_remove(row);
        Some(self.state.swap_remove(row))
    }

    #[must_use]
    pub fn ids(&self) -> &[EntityId] {
        &self.id
    }
}
