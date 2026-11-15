use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

/// The player enters the minigame.
#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnterGame {
    pub entity: Entity,
}
