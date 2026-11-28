use std::collections::BTreeMap;
use std::mem;

use bevy_ecs::resource::Resource;
use mcrs_engine_cell::CellId;

use mcrs_minecraft_engine_world::entity_ids::EntityIdRange;

#[derive(Resource, Debug)]
pub struct EntityIdRanges {
    next: u64,
    granted: BTreeMap<CellId, EntityIdRange>,
}

impl Default for EntityIdRanges {
    fn default() -> Self {
        Self {
            next: 2,
            granted: BTreeMap::new(),
        }
    }
}

impl EntityIdRanges {
    pub(crate) fn grant(&mut self) -> Option<EntityIdRange> {
        let range = EntityIdRange::starting_at(self.next)?;
        self.next = range.end();
        Some(range)
    }

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

    pub(crate) fn take_granted(&mut self) -> BTreeMap<CellId, EntityIdRange> {
        mem::take(&mut self.granted)
    }
}

#[cfg(test)]
mod tests {
    use mcrs_engine_muxer::EntityId;

    use super::*;
    use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;

    #[test]
    fn first_granted_id_is_not_zero() {
        let mut ids = CellEntityIds::default();
        ids.add(EntityIdRanges::default().grant().expect("a fresh range"));
        assert_eq!(ids.allocate(), Some(EntityId(2)));
    }
}
