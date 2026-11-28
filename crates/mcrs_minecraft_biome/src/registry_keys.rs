use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BIOME: RegistryKey<crate::Biome> = <crate::Biome as Registered>::REGISTRY;
impl Registered for crate::Biome {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/biome"));
}

pub const BIOME_SOURCE: RegistryKey<crate::biome_source_keys::BiomeSourceType> =
    <crate::biome_source_keys::BiomeSourceType as Registered>::REGISTRY;
impl Registered for crate::biome_source_keys::BiomeSourceType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/biome_source"));
}

pub const CARVER: RegistryKey<crate::ConfiguredCarver> =
    <crate::ConfiguredCarver as Registered>::REGISTRY;
impl Registered for crate::ConfiguredCarver {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/carver"));
}

pub const MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST: RegistryKey<
    crate::parameter_list::MultiNoiseBiomeSourceParameterList,
> = <crate::parameter_list::MultiNoiseBiomeSourceParameterList as Registered>::REGISTRY;
impl Registered for crate::parameter_list::MultiNoiseBiomeSourceParameterList {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!(
        "minecraft:worldgen/multi_noise_biome_source_parameter_list"
    ));
}

pub const PLACED_FEATURE: RegistryKey<crate::PlacedFeature> =
    <crate::PlacedFeature as Registered>::REGISTRY;
impl Registered for crate::PlacedFeature {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/placed_feature"));
}
