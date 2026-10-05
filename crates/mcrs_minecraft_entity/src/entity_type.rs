use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, rl};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityType;

impl RegistryKey for EntityType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:entity_type");
}
