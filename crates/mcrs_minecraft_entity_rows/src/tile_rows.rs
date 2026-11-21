use core::iter;
use core::num::NonZeroU32;

use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::MotionRow;
use rustc_hash::FxHashMap;

use crate::MobRows;

#[derive(Clone, Debug, PartialEq)]
pub struct TileRows<M> {
    state: Vec<EntityState>,
    motion: Vec<MotionRow>,
    id: Vec<EntityId>,
    mob_slot: Vec<Option<NonZeroU32>>,
    mobs: Vec<M>,
    mob_row: Vec<usize>,
    rows: FxHashMap<EntityId, usize>,
    free_ids: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row<M> {
    pub state: EntityState,
    pub motion: MotionRow,
    pub id: EntityId,
    pub mob: Option<M>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowRef<'a> {
    pub state: &'a EntityState,
    pub motion: &'a MotionRow,
    pub id: EntityId,
}

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

    pub(crate) fn set_mob(&mut self, id: EntityId, mob: M) {
        let Some(row) = self.row_of(id) else {
            return;
        };
        match self.slot_at(row).and_then(|slot| self.mobs.get_mut(slot)) {
            Some(held) => *held = mob,
            None => self.attach(row, mob),
        }
    }

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

    #[must_use]
    pub fn states(&self) -> &[EntityState] {
        &self.state
    }

    #[must_use]
    pub fn mob(&self, id: EntityId) -> Option<&M> {
        let slot = self.slot_at(self.row_of(id)?)?;
        self.mobs.get(slot)
    }

    #[must_use]
    pub fn mob_count(&self) -> usize {
        self.mobs.len()
    }

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

    pub fn rows(&self) -> impl Iterator<Item = RowRef<'_>> {
        iter::zip(
            iter::zip(&self.state, &self.motion),
            self.id.iter().copied(),
        )
        .map(|((state, motion), id)| RowRef { state, motion, id })
    }

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
    use mcrs_minecraft_entity_key::EntityUuid;
    use mcrs_minecraft_physics::Body;

    use super::*;

    fn state(x: f64) -> EntityState {
        EntityState::new(
            EntityUuid::from_u128(0),
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
    fn rows_keep_id_and_mob_column_after_despawns_mid_and_end() {
        let mut rows: TileRows<u64> = TileRows::default();
        for index in 0_u32..6 {
            let key = EntityUuid::from_u128(u128::from(index));
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
