// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod particle_type;
pub mod position_source_type;

pub use particle_type::ParticleType;
pub use position_source_type::PositionSourceType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const PARTICLE_TYPE: RegistryKey<crate::keys::ParticleType> = RegistryKey::new(rl!("minecraft:particle_type"));
impl Registered for crate::keys::ParticleType {
    const REGISTRY: RegistryKey<Self> = PARTICLE_TYPE;
}

pub const POSITION_SOURCE_TYPE: RegistryKey<crate::keys::PositionSourceType> = RegistryKey::new(rl!("minecraft:position_source_type"));
impl Registered for crate::keys::PositionSourceType {
    const REGISTRY: RegistryKey<Self> = POSITION_SOURCE_TYPE;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        PARTICLE_TYPE.binding(),
        POSITION_SOURCE_TYPE.binding(),
    ]
}
