use bevy_ecs::entity::Entity;
use mcrs_minecraft_entity_key::EntityId;

/// An entity with its network id, which orders the events of one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Subject {
    pub(crate) id: EntityId,
    pub(crate) entity: Entity,
}
