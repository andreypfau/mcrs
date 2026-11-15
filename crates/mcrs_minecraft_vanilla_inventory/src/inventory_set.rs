use bevy_ecs::schedule::SystemSet;

/// The systems of [`VanillaInventoryPlugin`](crate::VanillaInventoryPlugin) in `FixedUpdate`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InventorySet {
    /// An edge applies the inputs of the players here, in the order of arrival, each request
    /// committed before the next input reads the slots. It is in
    /// [`TickSet::ClientInput`](mcrs_minecraft_engine_world::tick::TickSet::ClientInput), so the
    /// simulation sees the committed slots and selection.
    Input,
    /// Makes an item entity from each stack that a drop takes out of a holder of a player. It runs
    /// after [`TickSet::Project`](mcrs_minecraft_engine_world::tick::TickSet::Project) and before
    /// [`TickSet::Output`](mcrs_minecraft_engine_world::tick::TickSet::Output).
    Drops,
}
