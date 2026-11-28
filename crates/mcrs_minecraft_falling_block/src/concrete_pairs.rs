use std::collections::BTreeMap;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;
use serde::Serialize;

pub const CONCRETE_PAIRS_PATH: &str = "mcrs/falling_block/concrete_pairs.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConcretePairsFile {
    #[serde(default)]
    pub replace: bool,
    pub values: BTreeMap<ResourceLocationBuf, ResourceLocationBuf>,
}
