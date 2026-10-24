// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod density_function;
pub mod density_function_type;

pub use density_function_type::DensityFunctionType;

use mcrs_minecraft_core::{Registered, RegistryKey, rl};

pub const DENSITY_FUNCTION: RegistryKey<crate::proto::DensityFunctionHolder> =
    RegistryKey::new(rl!("minecraft:worldgen/density_function"));
impl Registered for crate::proto::DensityFunctionHolder {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION;
}

pub const DENSITY_FUNCTION_TYPE: RegistryKey<crate::keys::DensityFunctionType> =
    RegistryKey::new(rl!("minecraft:worldgen/density_function_type"));
impl Registered for crate::keys::DensityFunctionType {
    const REGISTRY: RegistryKey<Self> = DENSITY_FUNCTION_TYPE;
}
