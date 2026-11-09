use core::iter;

use mcrs_minecraft_entity_key::EntityId;
use rustc_hash::FxHashMap;

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
    /// The position of each id in the columns.
    rows: FxHashMap<EntityId, usize>,
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
        self.rows.insert(row.id, self.id.len());
        self.state.push(row.state);
        self.motion.push(row.motion);
        self.id.push(row.id);
    }

    /// Removes the row of `id` and gives it; `None` when the tile has no such row.
    pub fn despawn_row(&mut self, id: EntityId) -> Option<Row> {
        let row = self.row_of(id)?;
        self.rows.remove(&id);
        let removed = Row {
            id: self.id.swap_remove(row),
            motion: self.motion.swap_remove(row),
            state: self.state.swap_remove(row),
        };
        if let Some(&moved) = self.id.get(row) {
            self.rows.insert(moved, row);
        }
        Some(removed)
    }

    /// The position of the row of `id` in [`Self::ids`]; `None` when the tile has no such row.
    #[must_use]
    pub fn row_of(&self, id: EntityId) -> Option<usize> {
        self.rows
            .get(&id)
            .copied()
            .filter(|&row| self.id.get(row) == Some(&id))
    }

    #[must_use]
    pub fn ids(&self) -> &[EntityId] {
        &self.id
    }

    /// The state of each row, in the order of [`Self::ids`].
    #[must_use]
    pub fn states(&self) -> &[EntityState] {
        &self.state
    }

    /// The row of `id`, open to its writer; `None` when the tile has no such row.
    pub fn row_mut(&mut self, id: EntityId) -> Option<RowMut<'_>> {
        let row = self.row_of(id)?;
        Some(RowMut {
            state: self.state.get_mut(row)?,
            motion: self.motion.get(row)?,
            id,
        })
    }

    pub fn rows_mut(&mut self) -> impl Iterator<Item = RowMut<'_>> {
        iter::zip(
            iter::zip(&mut self.state, &self.motion),
            self.id.iter().copied(),
        )
        .map(|((state, motion), id)| RowMut { state, motion, id })
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_entity_key::BirthKey;
    use mcrs_minecraft_entity_key::SourceKind;

    use super::*;
    use crate::Body;

    #[test]
    fn each_row_is_found_by_its_id_after_despawns_from_the_middle_and_the_end() {
        let mut rows = TileRows::default();
        for index in 0..6 {
            let key = BirthKey::new(0, SourceKind::Spawn, 0, index);
            rows.insert_row(Row {
                state: EntityState::new(key, Body::default(), None),
                motion: MotionRow::item(),
                id: EntityId(u64::from(index) + 10),
            });
        }
        for gone in [EntityId(11), EntityId(15), EntityId(10)] {
            assert_eq!(rows.despawn_row(gone).map(|row| row.id), Some(gone));
            assert_eq!(rows.row_of(gone), None, "{gone:?} left the tile");
        }
        for (at, &id) in rows.ids().iter().enumerate() {
            assert_eq!(rows.row_of(id), Some(at), "the index follows each swap");
        }
        assert_eq!(rows.ids().len(), 3);
    }
}
