// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const PARTICLE_TYPE: RegistryKey<crate::particle_type_keys::ParticleType> =
    RegistryKey::new(rl!("minecraft:particle_type"));
impl Registered for crate::particle_type_keys::ParticleType {
    const REGISTRY: RegistryKey<Self> = PARTICLE_TYPE;
}

pub const POSITION_SOURCE_TYPE: RegistryKey<crate::position_source_type_keys::PositionSourceType> =
    RegistryKey::new(rl!("minecraft:position_source_type"));
impl Registered for crate::position_source_type_keys::PositionSourceType {
    const REGISTRY: RegistryKey<Self> = POSITION_SOURCE_TYPE;
}
