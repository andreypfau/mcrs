use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;

/// The network id of a moving entity. [`EntityMotionPlugin`](crate::EntityMotionPlugin) gives it
/// once to each entity with a [`MotionState`](crate::MotionState) and no id, and takes it back
/// when the entity loses its motion. An entity without an id does not move.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MotionId(pub EntityId);
