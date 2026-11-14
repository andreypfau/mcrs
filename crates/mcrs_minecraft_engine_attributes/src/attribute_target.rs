use bevy_ecs::entity::Entity;
use mcrs_minecraft_entity_key::EntityId;

/// The entity whose attribute instance an edit changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttributeTarget {
    /// A player entity of the world.
    Player(Entity),
    /// A mob row of the world.
    Row(EntityId),
}
