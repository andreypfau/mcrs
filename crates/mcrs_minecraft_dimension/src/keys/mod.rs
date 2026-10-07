// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod chunk_generator;
pub mod dimension;
pub mod dimension_type;

pub use chunk_generator::ChunkGeneratorType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const DIMENSION: RegistryKey<crate::Dimension> = RegistryKey::new(rl!("minecraft:dimension"));
impl Registered for crate::Dimension {
    const REGISTRY: RegistryKey<Self> = DIMENSION;
}

pub const DIMENSION_TYPE: RegistryKey<crate::DimensionType> = RegistryKey::new(rl!("minecraft:dimension_type"));
impl Registered for crate::DimensionType {
    const REGISTRY: RegistryKey<Self> = DIMENSION_TYPE;
}

pub const CHUNK_GENERATOR: RegistryKey<crate::keys::ChunkGeneratorType> = RegistryKey::new(rl!("minecraft:worldgen/chunk_generator"));
impl Registered for crate::keys::ChunkGeneratorType {
    const REGISTRY: RegistryKey<Self> = CHUNK_GENERATOR;
}

pub fn bindings() -> [TypeBinding; 3] {
    [
        DIMENSION.binding(),
        DIMENSION_TYPE.binding(),
        CHUNK_GENERATOR.binding(),
    ]
}
