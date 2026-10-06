// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod noise_settings;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const NOISE_SETTINGS: RegistryKey<crate::router::NoiseGeneratorSettings> = RegistryKey::new(rl!("minecraft:worldgen/noise_settings"));
impl Registered for crate::router::NoiseGeneratorSettings {
    const REGISTRY: RegistryKey<Self> = NOISE_SETTINGS;
}

pub fn bindings() -> [TypeBinding; 1] {
    [
        NOISE_SETTINGS.binding(),
    ]
}
