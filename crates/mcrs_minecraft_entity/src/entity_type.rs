use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_core::{ResourceLocation, rl};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityType {
    pub identifier: ResourceLocation<&'static str>,
}

impl EntityType {
    pub const fn new(identifier: ResourceLocation<&'static str>) -> Self {
        Self { identifier }
    }
}

impl TaggedRegistry for EntityType {
    const REGISTRY_PATH: &'static str = "entity_type";
}

impl RegistryKey for EntityType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:entity_type");
}
