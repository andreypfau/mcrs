use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_wire::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(deny_unknown_fields)]
pub struct BannerPattern {
    pub asset_id: ResourceLocationBuf,
    pub translation_key: String,
}

mcrs_minecraft_registries::resolve_as_is!(BannerPattern);
