// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BIOME: RegistryKey<crate::Biome> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
impl Registered for crate::Biome {
    const REGISTRY: RegistryKey<Self> = BIOME;
}

pub const BIOME_SOURCE: RegistryKey<crate::biome_source_keys::BiomeSourceType> =
    RegistryKey::new(rl!("minecraft:worldgen/biome_source"));
impl Registered for crate::biome_source_keys::BiomeSourceType {
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
