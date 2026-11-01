// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK: RegistryKey<crate::block_keys::Block> = RegistryKey::new(rl!("minecraft:block"));
impl Registered for crate::block_keys::Block {
    const REGISTRY: RegistryKey<Self> = BLOCK;
}

pub const BLOCK_ENTITY_TYPE: RegistryKey<crate::block_entity_type_keys::BlockEntityType> =
    RegistryKey::new(rl!("minecraft:block_entity_type"));
impl Registered for crate::block_entity_type_keys::BlockEntityType {
    const REGISTRY: RegistryKey<Self> = BLOCK_ENTITY_TYPE;
}

pub const FLUID: RegistryKey<crate::fluid_keys::Fluid> = RegistryKey::new(rl!("minecraft:fluid"));
impl Registered for crate::fluid_keys::Fluid {
    const REGISTRY: RegistryKey<Self> = FLUID;
}
