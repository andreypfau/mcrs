use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::registry_key::RegistryValue;
use mcrs_minecraft_keys as keys;
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::{Holder, HolderWireOnly};
use crate::harness::Sample;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintingVariantValue {
    pub width: Bounded<1, 16, 1>,
    pub height: Bounded<1, 16, 1>,
    pub asset_id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Text>,
}

impl RegistryValue for PaintingVariantValue {
    type Registry = keys::PaintingVariant;
}

/// The persistent form is the registry id alone; the wire form carries the
/// variant inline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaintingVariant(pub HolderWireOnly<PaintingVariantValue>);

impl Sample for PaintingVariant {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![PaintingVariant(HolderWireOnly(Holder::reference(
            ResourceLocation::minecraft("kebab"),
        )))]
    }
}
