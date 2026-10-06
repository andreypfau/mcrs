// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod biome;
pub mod biome_tags;
pub mod multi_noise_biome_source_parameter_list;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const BIOME: RegistryKey<crate::Biome> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
impl Registered for crate::Biome {
    const REGISTRY: RegistryKey<Self> = BIOME;
}

pub const MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST: RegistryKey<crate::parameter_list::MultiNoiseBiomeSourceParameterList> = RegistryKey::new(rl!("minecraft:worldgen/multi_noise_biome_source_parameter_list"));
impl Registered for crate::parameter_list::MultiNoiseBiomeSourceParameterList {
    const REGISTRY: RegistryKey<Self> = MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        BIOME.binding(),
        MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST.binding(),
    ]
}
