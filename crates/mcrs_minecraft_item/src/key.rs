use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, rl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Potion {}

impl RegistryKey for Potion {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:potion");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Recipe {}

impl RegistryKey for Recipe {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:recipe");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootTable {}

impl RegistryKey for LootTable {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:loot_table");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapDecorationType {}

impl RegistryKey for MapDecorationType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:map_decoration_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Menu {}

impl RegistryKey for Menu {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:menu");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextIntProvider {}

impl RegistryKey for ContextIntProvider {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:context_int_provider");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextFloatProvider {}

impl RegistryKey for ContextFloatProvider {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:context_float_provider");
}
