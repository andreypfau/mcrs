use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::tag_key::TaggedRegistry;
use mcrs_minecraft_core::{ResourceLocation, rl};

/// The block registry, as the tag system names it. The blocks themselves live
/// in the definition corpus and are addressed by their index in it, so this
/// type carries no value — it only says which registry a `TagKey` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {}

impl TaggedRegistry for Block {
    const REGISTRY_PATH: &'static str = "block";
}

impl RegistryKey for Block {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:block");
}

/// The fluid registry, as the tag system names it. The fluids are the ones the
/// block corpus interns, addressed by their index there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fluid {}

impl TaggedRegistry for Fluid {
    const REGISTRY_PATH: &'static str = "fluid";
}

impl RegistryKey for Fluid {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:fluid");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialog {}

impl TaggedRegistry for Dialog {
    const REGISTRY_PATH: &'static str = "dialog";
}

impl RegistryKey for Dialog {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:dialog");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {}

impl TaggedRegistry for Biome {
    const REGISTRY_PATH: &'static str = "worldgen/biome";
}

impl RegistryKey for Biome {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:worldgen/biome");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Structure {}

impl TaggedRegistry for Structure {
    const REGISTRY_PATH: &'static str = "worldgen/structure";
}

impl RegistryKey for Structure {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:worldgen/structure");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockTransformer {}

impl RegistryKey for BlockTransformer {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:block_transformer");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecoratedPotPattern {}

impl RegistryKey for DecoratedPotPattern {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:decorated_pot_pattern");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockEntityType {}

impl RegistryKey for BlockEntityType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:block_entity_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {}

impl RegistryKey for Dimension {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:dimension");
}
