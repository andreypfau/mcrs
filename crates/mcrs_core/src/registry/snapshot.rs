use bevy_ecs::resource::Resource;
use std::collections::HashMap;

/// Stable `u32` network IDs assigned to all dynamic registry entries once
/// `AppState::WorldgenFreeze` is entered.
///
/// These IDs are sent to clients in the Configuration phase via
/// `ClientboundRegistryDataPacket`.  They stay constant for the lifetime of
/// the running server (re-assigned on reconfiguration).
///
/// This is currently a stub — full population logic lives in `mc_server`.
#[derive(Resource, Debug, Default)]
pub struct RegistrySnapshot {
    /// registry key (e.g. "minecraft:worldgen/biome") → (entry RL → protocol id)
    pub entries: HashMap<String, HashMap<String, u32>>,
}

impl RegistrySnapshot {
    pub fn new() -> Self {
        RegistrySnapshot::default()
    }
}
