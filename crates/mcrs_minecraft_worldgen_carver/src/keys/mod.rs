// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod carver;
pub mod carver_type;

pub use carver_type::CarverType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const CARVER: RegistryKey<crate::config::CarverConfig> = RegistryKey::new(rl!("minecraft:worldgen/carver"));
impl Registered for crate::config::CarverConfig {
    const REGISTRY: RegistryKey<Self> = CARVER;
}

pub const CARVER_TYPE: RegistryKey<crate::keys::CarverType> = RegistryKey::new(rl!("minecraft:worldgen/carver_type"));
impl Registered for crate::keys::CarverType {
    const REGISTRY: RegistryKey<Self> = CARVER_TYPE;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        CARVER.binding(),
        CARVER_TYPE.binding(),
    ]
}
