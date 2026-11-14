use std::collections::BTreeMap;

use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;
use serde::Serialize;

/// The path of the concrete pairs inside a data pack, under the namespace `mcrs` (FBK-TRG-002).
pub const CONCRETE_PAIRS_PATH: &str = "mcrs/falling_block/concrete_pairs.json";

/// The file of the concrete pairs of one data pack (FBK-TRG-002).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConcretePairsFile {
    /// True discards the pairs of the packs that load before this pack.
    #[serde(default)]
    pub replace: bool,
    /// Each key is a powder block; its value is the concrete block.
    pub values: BTreeMap<ResourceLocationBuf, ResourceLocationBuf>,
}

/// The concrete pairs of one data pack, with the name of the pack for an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackPairs<'a> {
    pub pack: &'a str,
    pub file: &'a ConcretePairsFile,
}
