use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const DIMENSION: RegistryKey<crate::Dimension> = <crate::Dimension as Registered>::REGISTRY;
impl Registered for crate::Dimension {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dimension"));
}

pub const DIMENSION_TYPE: RegistryKey<crate::DimensionType> =
    <crate::DimensionType as Registered>::REGISTRY;
impl Registered for crate::DimensionType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dimension_type"));
}

pub const CHUNK_GENERATOR: RegistryKey<crate::chunk_generator_keys::ChunkGeneratorType> =
    <crate::chunk_generator_keys::ChunkGeneratorType as Registered>::REGISTRY;
impl Registered for crate::chunk_generator_keys::ChunkGeneratorType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/chunk_generator"));
}

pub const NOISE_SETTINGS: RegistryKey<crate::NoiseSettings> =
    <crate::NoiseSettings as Registered>::REGISTRY;
impl Registered for crate::NoiseSettings {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/noise_settings"));
}
