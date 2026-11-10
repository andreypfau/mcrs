use core::iter;
use core::num::NonZeroU32;

use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use rustc_hash::FxHashMap;

use crate::MobRows;

/// The rows of the server entities of one tile, one column per field. A spawn or a despawn of an
/// item, an orb or a projectile is a row operation here and never touches ECS; tiles do them in
/// parallel. A despawn moves the last row into the gap, so the row order is not kept.
///
/// The mob column `M` is dense: only the rows that carry one hold its bytes, so items and orbs
/// pay four bytes for it, and it moves with its row from tile to tile.
#[derive(Clone, Debug, PartialEq)]
pub struct TileRows<M> {
    state: Vec<EntityState>,
    motion: Vec<MotionRow>,
    id: Vec<EntityId>,
    /// The slot in `mobs` of each row, plus one; `None` for a row without a mob column.
    mob_slot: Vec<Option<NonZeroU32>>,
    mobs: Vec<M>,
    /// The position in the columns of the row of each mob.
    mob_row: Vec<usize>,
    /// The position of each id in the columns.
    rows: FxHashMap<EntityId, usize>,
    free_ids: Vec<EntityId>,
}

/// One row, out of its tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row<M> {
    pub state: EntityState,
    pub motion: MotionRow,
    pub id: EntityId,
    pub mob: Option<M>,
}

/// One row in its tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowRef<'a> {
    pub state: &'a EntityState,
    pub motion: &'a MotionRow,
    pub id: EntityId,
}

/// One row in its tile, with its state open to the writer of the phase.
#[derive(Debug, PartialEq)]
pub(crate) struct RowMut<'a> {
    pub(crate) state: &'a mut EntityState,
    pub(crate) motion: &'a MotionRow,
    pub(crate) id: EntityId,
}

impl<M> Default for TileRows<M> {
    fn default() -> Self {
        Self {
            state: Vec::new(),
            motion: Vec::new(),
            id: Vec::new(),
            mob_slot: Vec::new(),
            mobs: Vec::new(),
            mob_row: Vec::new(),
            rows: FxHashMap::default(),
            free_ids: Vec::new(),
        }
    }
}

impl<M> TileRows<M> {
    /// Gives the tile ids from the range of its cell, for the spawns of later ticks.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the rows that a tile task spawns take reserved ids"
        )
    )]
    pub(crate) fn reserve_ids(&mut self, ids: impl IntoIterator<Item = EntityId>) {
        self.free_ids.extend(ids);
    }

    /// Adds a row with one of the reserved ids and no mob column; `None` when the tile has no
    /// reserved id left.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the rows that a tile task spawns take reserved ids"
        )
    )]
    pub(crate) fn spawn_row(&mut self, state: EntityState, motion: MotionRow) -> Option<EntityId> {
        let id = self.free_ids.pop()?;
        self.insert_row(Row {
            state,
            motion,
            id,
            mob: None,
        });
        Some(id)
    }

    /// Adds a row that keeps its id, such as a row that comes from another tile.
    pub(crate) fn insert_row(&mut self, row: Row<M>) {
        let at = self.id.len();
        self.rows.insert(row.id, at);
        self.state.push(row.state);
        self.motion.push(row.motion);
        self.id.push(row.id);
        self.mob_slot.push(None);
        if let Some(mob) = row.mob {
            self.attach(at, mob);
        }
    }

    /// Removes the row of `id` and gives it, with its mob column; `None` when the tile has no
    /// such row.
    pub(crate) fn despawn_row(&mut self, id: EntityId) -> Option<Row<M>> {
        let row = self.row_of(id)?;
        self.rows.remove(&id);
        let mob = self.detach(row);
        let removed = Row {
            id: self.id.swap_remove(row),
            motion: self.motion.swap_remove(row),
            state: self.state.swap_remove(row),
            mob,
        };
        self.mob_slot.swap_remove(row);
        if let Some(&moved) = self.id.get(row) {
            self.rows.insert(moved, row);
            if let Some(slot) = self.slot_at(row)
                && let Some(mob_row) = self.mob_row.get_mut(slot)
            {
                *mob_row = row;
            }
        }
        Some(removed)
    }

    /// Gives the row of `id` the mob column `mob`, in place of the one it had. The tile drops
    /// `mob` when it has no such row.
    pub(crate) fn set_mob(&mut self, id: EntityId, mob: M) {
        let Some(row) = self.row_of(id) else {
            return;
        };
        match self.slot_at(row).and_then(|slot| self.mobs.get_mut(slot)) {
            Some(held) => *held = mob,
            None => self.attach(row, mob),
        }
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

    /// The mob column of the row of `id`; `None` when the tile has no such row or the row has
    /// no mob column.
    #[must_use]
    pub fn mob(&self, id: EntityId) -> Option<&M> {
        let slot = self.slot_at(self.row_of(id)?)?;
        self.mobs.get(slot)
    }

    /// The count of the rows with a mob column.
    #[must_use]
    pub fn mob_count(&self) -> usize {
        self.mobs.len()
    }

    /// The mob columns of the tile, open to their writer, beside the states of their rows, which
    /// stay read-only.
    pub fn mobs_mut(&mut self) -> MobRows<'_, M> {
        MobRows {
            ids: &self.id,
            states: &self.state,
            rows: &self.rows,
            mob_slot: &self.mob_slot,
            mob_row: &self.mob_row,
            mobs: &mut self.mobs,
        }
    }

    /// Each row, in the order of [`Self::ids`].
    pub fn rows(&self) -> impl Iterator<Item = RowRef<'_>> {
        iter::zip(
            iter::zip(&self.state, &self.motion),
            self.id.iter().copied(),
        )
        .map(|((state, motion), id)| RowRef { state, motion, id })
    }

    /// The row of `id`, open to its writer; `None` when the tile has no such row.
    pub(crate) fn row_mut(&mut self, id: EntityId) -> Option<RowMut<'_>> {
        let row = self.row_of(id)?;
        Some(RowMut {
            state: self.state.get_mut(row)?,
            motion: self.motion.get(row)?,
            id,
        })
    }

    pub(crate) fn rows_mut(&mut self) -> impl Iterator<Item = RowMut<'_>> {
        iter::zip(
            iter::zip(&mut self.state, &self.motion),
            self.id.iter().copied(),
        )
        .map(|((state, motion), id)| RowMut { state, motion, id })
    }

    fn slot_at(&self, row: usize) -> Option<usize> {
        let slot = (*self.mob_slot.get(row)?)?;
        usize::try_from(slot.get() - 1).ok()
    }

    /// Adds `mob` as the column of the row at `row`, which has none. A tile with `u32::MAX`
    /// mobs drops it.
    fn attach(&mut self, row: usize, mob: M) {
        let Some(slot) = u32::try_from(self.mobs.len())
            .ok()
            .and_then(|slot| NonZeroU32::new(slot.wrapping_add(1)))
        else {
            return;
        };
        let Some(held) = self.mob_slot.get_mut(row) else {
            return;
        };
        *held = Some(slot);
        self.mobs.push(mob);
        self.mob_row.push(row);
    }

    /// Takes the mob column of the row at `row` out; the last mob moves into its slot.
    fn detach(&mut self, row: usize) -> Option<M> {
        let slot = self.slot_at(row)?;
        if slot >= self.mobs.len() {
            return None;
        }
        if let Some(held) = self.mob_slot.get_mut(row) {
            *held = None;
        }
        let mob = self.mobs.swap_remove(slot);
        self.mob_row.swap_remove(slot);
        if let Some(&moved) = self.mob_row.get(slot)
            && let Some(held) = self.mob_slot.get_mut(moved)
        {
            *held = u32::try_from(slot)
                .ok()
                .and_then(|slot| NonZeroU32::new(slot.wrapping_add(1)));
        }
        Some(mob)
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use mcrs_minecraft_entity_key::EntityKey;
    use mcrs_minecraft_physics::Body;

    use super::*;

    fn state(x: f64) -> EntityState {
        EntityState::new(
            EntityKey(0),
            Body {
                position: DVec3::new(x, 0.0, 0.0),
                ..Body::default()
            },
            None,
        )
    }

    #[test]
    fn rows_spawn_with_reserved_ids_and_despawn_by_id() {
        let mut rows: TileRows<()> = TileRows::default();
        assert_eq!(
            rows.spawn_row(state(0.0), MotionRow::item()),
            None,
            "no id reserved"
        );
        rows.reserve_ids([EntityId(10), EntityId(12)]);
        let first = rows
            .spawn_row(state(1.0), MotionRow::item())
            .expect("a reserved id");
        let second = rows
            .spawn_row(state(2.0), MotionRow::item())
            .expect("a reserved id");
        assert_ne!(first, second);
        assert_eq!(
            rows.spawn_row(state(3.0), MotionRow::item()),
            None,
            "both ids are used"
        );

        assert_eq!(
            rows.despawn_row(first).map(|row| row.state),
            Some(state(1.0))
        );
        assert_eq!(rows.despawn_row(first), None, "the row is gone");
        assert_eq!(rows.ids(), &[second]);
    }

    #[test]
    fn each_row_keeps_its_id_and_its_mob_column_after_despawns_from_the_middle_and_the_end() {
        let mut rows: TileRows<u64> = TileRows::default();
        for index in 0_u32..6 {
            let key = EntityKey(u128::from(index));
            let id = u64::from(index) + 10;
            rows.insert_row(Row {
                state: EntityState::new(key, Body::default(), None),
                motion: MotionRow::item(),
                id: EntityId(id),
                mob: (index % 2 == 1).then_some(id),
            });
        }
        rows.set_mob(EntityId(12), 12);
        for gone in [EntityId(11), EntityId(15), EntityId(10)] {
            let row = rows.despawn_row(gone);
            assert_eq!(row.map(|row| row.id), Some(gone));
            assert_eq!(
                row.and_then(|row| row.mob),
                (gone != EntityId(10)).then_some(gone.0),
                "the mob column leaves with its row"
            );
            assert_eq!(rows.row_of(gone), None, "{gone:?} left the tile");
        }
        for (at, &id) in rows.ids().iter().enumerate() {
            assert_eq!(rows.row_of(id), Some(at), "the index follows each swap");
        }
        assert_eq!(rows.ids().len(), 3);
        for (id, mob) in [(12, Some(12)), (13, Some(13)), (14, None)] {
            assert_eq!(
                rows.mob(EntityId(id)).copied(),
                mob,
                "row {id} keeps its own mob column"
            );
        }
        let mut seen: Vec<(EntityId, u64)> = rows
            .mobs_mut()
            .iter_mut()
            .map(|row| (row.id, *row.mob))
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, [(EntityId(12), 12), (EntityId(13), 13)]);
    }
}
