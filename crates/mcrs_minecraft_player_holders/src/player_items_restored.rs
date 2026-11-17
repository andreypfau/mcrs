use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerItemsRestored {
    pub entity: Entity,
}
