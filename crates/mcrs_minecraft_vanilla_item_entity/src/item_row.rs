use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

/// The physics row of a dropped stack: its network id and its UUID.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ItemRow {
    pub(crate) id: EntityId,
    pub(crate) key: EntityKey,
}
