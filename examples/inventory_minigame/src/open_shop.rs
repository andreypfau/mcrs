use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

/// The shop menu opens for the player.
#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenShop {
    pub entity: Entity,
}
