use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::item::{ComponentMap, Template};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinitionFile {
    pub format_version: String,
    #[serde(rename = "minecraft:item")]
    pub item: ItemDefinition,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinition {
    pub description: Description,
    pub components: ComponentMap,
    #[serde(default)]
    pub block_placer: Option<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub crafting_remainder: Option<Template>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub identifier: ResourceLocation<Arc<str>>,
    pub protocol_id: u32,
}
