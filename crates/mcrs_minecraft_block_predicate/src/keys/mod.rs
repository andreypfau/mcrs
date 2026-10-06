// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod block_state_provider;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const BLOCK_STATE_PROVIDER: RegistryKey<crate::provider::DirectBlockStateProvider> = RegistryKey::new(rl!("minecraft:worldgen/block_state_provider"));
impl Registered for crate::provider::DirectBlockStateProvider {
    const REGISTRY: RegistryKey<Self> = BLOCK_STATE_PROVIDER;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        BLOCK_STATE_PROVIDER.binding(),
    ]
}
