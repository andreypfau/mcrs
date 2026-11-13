use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

/// The holders of a player beside its player inventory, which is the
/// [`Slots`](mcrs_minecraft_engine_item::Slots) of the player itself (INV-SLT-002). Each one is a
/// child of the player with its own `Slots`: one slot for the cursor, four for the craft grid in
/// rows from the top left, and one for the craft result.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerHolders {
    pub cursor: Entity,
    pub craft_grid: Entity,
    pub craft_result: Entity,
}
