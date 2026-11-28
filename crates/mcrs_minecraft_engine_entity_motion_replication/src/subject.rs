use bevy_ecs::entity::Entity;
use mcrs_minecraft_entity_key::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Subject {
    pub(crate) id: EntityId,
    pub(crate) entity: Entity,
}
