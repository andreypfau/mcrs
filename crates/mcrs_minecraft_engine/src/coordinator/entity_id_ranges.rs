use std::collections::BTreeMap;
use std::mem;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

use crate::entity_ids::EntityIdRange;

/// The global entity ids that no cell holds yet, and the ranges granted for the next tick of
/// each dimension, by its label. The ranges never return, so no two cells share an id.
#[derive(Resource, Debug, Default)]
pub struct EntityIdRanges {
    next: u64,
    granted: BTreeMap<Entity, EntityIdRange>,
}

impl EntityIdRanges {
    /// Takes the next range out of the free ids. Gives `None` when no free ids remain.
    pub(crate) fn grant(&mut self) -> Option<EntityIdRange> {
        let range = EntityIdRange::starting_at(self.next)?;
        self.next = range.end();
        Some(range)
    }

    /// Answers the request of the dimension of `label`: its next input carries a new range.
    /// Requests from more than one output before that input get one range.
    pub(crate) fn answer(&mut self, label: Entity) {
        if self.granted.contains_key(&label) {
            return;
        }
        let Some(range) = self.grant() else {
            tracing::error!(?label, "no global entity ids remain to grant");
            return;
        };
        self.granted.insert(label, range);
    }

    /// The ranges for the next tick, by label. A range for a dimension that closed is lost.
    pub(crate) fn take_granted(&mut self) -> BTreeMap<Entity, EntityIdRange> {
        mem::take(&mut self.granted)
    }
}
