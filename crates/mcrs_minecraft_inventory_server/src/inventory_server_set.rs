use bevy_ecs::schedule::SystemSet;

/// The systems of [`InventoryServerPlugin`](crate::InventoryServerPlugin) in `FixedUpdate`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InventoryServerSet {
    /// Sends the stacks that the client does not hold, and keeps the client view and the state
    /// id of the inventory menu. It runs after
    /// [`TickSet::Project`](mcrs_minecraft_engine_world::tick::TickSet::Project) and
    /// [`InventorySet::Drops`](mcrs_minecraft_vanilla_inventory::InventorySet::Drops), and before
    /// [`TickSet::Output`](mcrs_minecraft_engine_world::tick::TickSet::Output).
    Sync,
}
