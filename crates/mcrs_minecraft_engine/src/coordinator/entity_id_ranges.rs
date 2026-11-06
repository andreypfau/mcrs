use std::collections::BTreeMap;
use std::mem;

use bevy_ecs::resource::Resource;
use mcrs_engine_cell::CellId;

use crate::entity_ids::EntityIdRange;

/// The global entity ids that no cell holds yet, and the ranges granted for the next tick of
/// each cell. The ranges never return, so no two cells share an id.
#[derive(Resource, Debug)]
pub struct EntityIdRanges {
    next: u64,
    granted: BTreeMap<CellId, EntityIdRange>,
}

impl Default for EntityIdRanges {
    /// Starts at 2: some packets use the id 0 to mean "no entity".
    fn default() -> Self {
        Self {
            next: 2,
            granted: BTreeMap::new(),
        }
    }
}

impl EntityIdRanges {
    /// Takes the next range out of the free ids. Gives `None` when no free ids remain.
    pub(crate) fn grant(&mut self) -> Option<EntityIdRange> {
        let range = EntityIdRange::starting_at(self.next)?;
        self.next = range.end();
        Some(range)
    }

    /// Answers the request of `cell`: its next input carries a new range. Requests from more
    /// than one output before that input get one range.
    pub(crate) fn answer(&mut self, cell: CellId) {
        if self.granted.contains_key(&cell) {
            return;
        }
        let Some(range) = self.grant() else {
            tracing::error!(%cell, "no global entity ids remain to grant");
            return;
        };
        self.granted.insert(cell, range);
    }

    /// The ranges for the next tick, by cell. A range for a cell that stopped is lost.
    pub(crate) fn take_granted(&mut self) -> BTreeMap<CellId, EntityIdRange> {
        mem::take(&mut self.granted)
    }
}

#[cfg(test)]
mod tests {
    use mcrs_engine_muxer::EntityId;

    use super::*;
    use crate::entity_ids::CellEntityIds;

    #[test]
    fn first_granted_id_is_not_zero() {
        let mut ids = CellEntityIds::default();
        ids.add(EntityIdRanges::default().grant().expect("a fresh range"));
        assert_eq!(ids.allocate(), Some(EntityId(2)));
    }
}
