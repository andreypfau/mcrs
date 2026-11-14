use bevy_ecs::schedule::SystemSet;

/// The systems of [`VanillaInventoryPlugin`](crate::VanillaInventoryPlugin) in `FixedUpdate`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InventorySet {
    /// Applies each [`InventoryInput`](crate::InventoryInput) in the order of arrival, each request
    /// committed before the next input reads the slots. It is in
    /// [`TickSet::ClientInput`](mcrs_minecraft_engine_world::tick::TickSet::ClientInput), so the
    /// simulation sees the committed slots and selection.
    Input,
    /// Sends the stacks that the client does not hold, and keeps the client view and the state
    /// id of the inventory menu. It runs after
    /// [`TickSet::Project`](mcrs_minecraft_engine_world::tick::TickSet::Project) and before
    /// [`TickSet::Output`](mcrs_minecraft_engine_world::tick::TickSet::Output).
    Sync,
}
