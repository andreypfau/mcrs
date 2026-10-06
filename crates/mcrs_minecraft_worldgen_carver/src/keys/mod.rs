// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod carver;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const CARVER: RegistryKey<crate::config::CarverConfig> = RegistryKey::new(rl!("minecraft:worldgen/carver"));
impl Registered for crate::config::CarverConfig {
    const REGISTRY: RegistryKey<Self> = CARVER;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        CARVER.binding(),
    ]
}
