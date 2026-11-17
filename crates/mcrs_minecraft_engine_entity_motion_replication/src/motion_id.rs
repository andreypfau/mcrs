use bevy_ecs::component::Component;
use mcrs_minecraft_entity_key::EntityId;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MotionId(pub EntityId);
