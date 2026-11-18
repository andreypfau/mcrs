use std::collections::BTreeMap;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawReport {
    pub(crate) item_features: BTreeMap<ResourceLocationBuf, Vec<ResourceLocationBuf>>,
    pub(crate) tabs: BTreeMap<ResourceLocationBuf, RawTab>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawTab {
    pub(crate) needs_operator: bool,
    pub(crate) tab_list: Vec<NbtCompound>,
    pub(crate) search_list: Vec<NbtCompound>,
}
