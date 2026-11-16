use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;

/// The stacks of a player came from a restore: the player file, a handover or
/// [`restore_items`](crate::PlayerInventoryCommandsExt::restore_items). A client of the player no
/// longer knows what its holders hold.
#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerItemsRestored {
    pub entity: Entity,
}
