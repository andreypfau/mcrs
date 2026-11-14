use std::collections::HashMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

/// The client entity of each network id.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct NetworkEntities(HashMap<i32, Entity>);

impl NetworkEntities {
    #[must_use]
    pub fn get(&self, network_id: i32) -> Option<Entity> {
        self.0.get(&network_id).copied()
    }

    pub(crate) fn insert(&mut self, network_id: i32, entity: Entity) -> Option<Entity> {
        self.0.insert(network_id, entity)
    }

    pub(crate) fn remove(&mut self, network_id: i32) -> Option<Entity> {
        self.0.remove(&network_id)
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.0.drain().map(|(_, entity)| entity)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
