use bevy_ecs::schedule::SystemSet;

/// The systems of [`VanillaInventoryPlugin`](crate::VanillaInventoryPlugin) in `FixedUpdate`, after
/// [`TickSet::Project`](mcrs_minecraft_engine_world::tick::TickSet::Project) and before
/// [`TickSet::Output`](mcrs_minecraft_engine_world::tick::TickSet::Output). `Input` runs before
/// `Sync`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InventorySet {
    /// Turns each [`InventoryClick`](crate::InventoryClick) into a request of the engine.
    Input,
    /// Sends the stacks that the client does not hold, and keeps the client view and the state
    /// id of the inventory menu.
    Sync,
}
