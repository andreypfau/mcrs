// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod dimension;
pub mod dimension_type;

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

pub fn bindings() -> [TypeBinding; 2] {
    [
        DIMENSION.binding(),
        DIMENSION_TYPE.binding(),
    ]
}
