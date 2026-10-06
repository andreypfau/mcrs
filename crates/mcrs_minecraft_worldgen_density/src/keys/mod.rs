// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod density_function_type;
pub mod noise_settings;

pub use density_function_type::DensityFunctionType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const DENSITY_FUNCTION_TYPE: RegistryKey<crate::keys::DensityFunctionType> = RegistryKey::new(rl!("minecraft:worldgen/density_function_type"));
impl Registered for crate::keys::DensityFunctionType {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION_TYPE;
}

pub const NOISE_SETTINGS: RegistryKey<crate::router::NoiseGeneratorSettings> = RegistryKey::new(rl!("minecraft:worldgen/noise_settings"));
impl Registered for crate::router::NoiseGeneratorSettings {
    const REGISTRY: RegistryKey<Self> = NOISE_SETTINGS;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        DENSITY_FUNCTION_TYPE.binding(),
        NOISE_SETTINGS.binding(),
    ]
}
