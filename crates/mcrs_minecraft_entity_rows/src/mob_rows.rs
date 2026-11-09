use core::num::NonZeroU32;

use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::EntityState;
use rustc_hash::FxHashMap;

/// The mob columns of one tile, open to their writer, beside the read-only states of their rows.
/// A writer of the mob columns cannot change a row, so the grid of the tile stays valid.
#[derive(Debug)]
pub struct MobRows<'a, M> {
    pub(crate) ids: &'a [EntityId],
    pub(crate) states: &'a [EntityState],
    pub(crate) rows: &'a FxHashMap<EntityId, usize>,
    pub(crate) mob_slot: &'a [Option<NonZeroU32>],
    pub(crate) mob_row: &'a [usize],
    pub(crate) mobs: &'a mut [M],
}

/// One row with a mob column.
#[derive(Debug)]
pub struct MobRow<'a, M> {
    pub id: EntityId,
    pub state: &'a EntityState,
    pub mob: &'a mut M,
}

impl<M> MobRows<'_, M> {
    /// The rows with a mob column, in the order of the mob slots.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = MobRow<'_, M>> {
        let ids = self.ids;
        let states = self.states;
        self.mobs
            .iter_mut()
            .zip(self.mob_row)
            .filter_map(move |(mob, &row)| {
                Some(MobRow {
                    id: *ids.get(row)?,
                    state: states.get(row)?,
                    mob,
                })
            })
    }

    /// The row of `id` with its mob column; `None` when the tile has no such row or the row has
    /// no mob column.
    pub fn get_mut(&mut self, id: EntityId) -> Option<MobRow<'_, M>> {
        let row = self
            .rows
            .get(&id)
            .copied()
            .filter(|&row| self.ids.get(row) == Some(&id))?;
        let slot = usize::try_from(self.mob_slot.get(row).copied()??.get() - 1).ok()?;
        Some(MobRow {
            id,
            state: self.states.get(row)?,
            mob: self.mobs.get_mut(slot)?,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.mobs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mobs.is_empty()
    }
}
