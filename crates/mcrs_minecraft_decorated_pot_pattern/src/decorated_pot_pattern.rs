use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecoratedPotPattern {
    pub asset_id: ResourceLocationBuf,
}

mcrs_minecraft_registries::resolve_as_is!(DecoratedPotPattern);
