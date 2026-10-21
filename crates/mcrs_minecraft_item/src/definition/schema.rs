use std::sync::Arc;

use crate::{ComponentMap, Template};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::Named;
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
    pub components: ComponentMap<Named>,
    #[serde(default)]
    pub block_placer: Option<ResourceLocation<Arc<str>>>,
    #[serde(default)]
    pub crafting_remainder: Option<Template<Named>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    pub identifier: ResourceLocation<Arc<str>>,
    pub protocol_id: u16,
}
