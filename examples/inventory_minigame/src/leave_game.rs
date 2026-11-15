use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

/// The player leaves the minigame.
#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaveGame {
    pub entity: Entity,
}
