// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod biome;
pub mod biome_source;
pub mod biome_tags;
pub mod carver;
pub mod multi_noise_biome_source_parameter_list;
pub mod placed_feature;

pub use biome_source::BiomeSourceType;

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BIOME: RegistryKey<crate::Biome> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
impl Registered for crate::Biome {
    const REGISTRY: RegistryKey<Self> = BIOME;
}

pub const BIOME_SOURCE: RegistryKey<crate::keys::BiomeSourceType> =
    RegistryKey::new(rl!("minecraft:worldgen/biome_source"));
impl Registered for crate::keys::BiomeSourceType {
    const REGISTRY: RegistryKey<Self> = BIOME_SOURCE;
}

pub const CARVER: RegistryKey<crate::ConfiguredCarver> =
    RegistryKey::new(rl!("minecraft:worldgen/carver"));
impl Registered for crate::ConfiguredCarver {
    const REGISTRY: RegistryKey<Self> = CARVER;
}

pub const MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST: RegistryKey<
    crate::parameter_list::MultiNoiseBiomeSourceParameterList,
> = RegistryKey::new(rl!(
    "minecraft:worldgen/multi_noise_biome_source_parameter_list"
));
impl Registered for crate::parameter_list::MultiNoiseBiomeSourceParameterList {
    const REGISTRY: RegistryKey<Self> = MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST;
}

pub const PLACED_FEATURE: RegistryKey<crate::PlacedFeature> =
    RegistryKey::new(rl!("minecraft:worldgen/placed_feature"));
impl Registered for crate::PlacedFeature {
    const REGISTRY: RegistryKey<Self> = PLACED_FEATURE;
}
