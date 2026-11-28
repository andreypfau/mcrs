use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BLOCK: RegistryKey<crate::block_keys::Block> =
    <crate::block_keys::Block as Registered>::REGISTRY;
impl Registered for crate::block_keys::Block {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:block"));
}

pub const BLOCK_ENTITY_TYPE: RegistryKey<crate::block_entity_type_keys::BlockEntityType> =
    <crate::block_entity_type_keys::BlockEntityType as Registered>::REGISTRY;
impl Registered for crate::block_entity_type_keys::BlockEntityType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:block_entity_type"));
}

pub const FLUID: RegistryKey<crate::fluid_keys::Fluid> =
    <crate::fluid_keys::Fluid as Registered>::REGISTRY;
impl Registered for crate::fluid_keys::Fluid {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:fluid"));
}
