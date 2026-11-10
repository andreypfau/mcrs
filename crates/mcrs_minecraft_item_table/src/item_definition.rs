use alloc::collections::BTreeMap;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

use crate::json_nbt::JsonNbt;

/// The layout of one file of the item definitions.
#[derive(Debug, Deserialize)]
pub(crate) struct ItemDefinition {
    #[serde(rename = "minecraft:item")]
    pub(crate) item: DefinedItem,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DefinedItem {
    pub(crate) description: Description,
    pub(crate) components: BTreeMap<ResourceLocationBuf, JsonNbt>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Description {
    pub(crate) identifier: ResourceLocationBuf,
    pub(crate) protocol_id: u16,
}
