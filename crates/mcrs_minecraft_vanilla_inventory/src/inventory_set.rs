use bevy_ecs::schedule::SystemSet;

/// The systems of [`VanillaInventoryPlugin`](crate::VanillaInventoryPlugin) in `FixedUpdate`, after
/// [`TickSet::Project`](mcrs_minecraft_engine_world::tick::TickSet::Project) and before
/// [`TickSet::Output`](mcrs_minecraft_engine_world::tick::TickSet::Output).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InventorySet;
