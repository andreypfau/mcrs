use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const PARTICLE_TYPE: RegistryKey<crate::particle_type_keys::ParticleType> =
    <crate::particle_type_keys::ParticleType as Registered>::REGISTRY;
impl Registered for crate::particle_type_keys::ParticleType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:particle_type"));
}

pub const POSITION_SOURCE_TYPE: RegistryKey<crate::position_source_type_keys::PositionSourceType> =
    <crate::position_source_type_keys::PositionSourceType as Registered>::REGISTRY;
impl Registered for crate::position_source_type_keys::PositionSourceType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:position_source_type"));
}
