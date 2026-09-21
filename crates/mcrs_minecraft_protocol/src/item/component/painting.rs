use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Holder, HolderWireOnly, PaintingVariantReg, Registered};
use crate::item::harness::Sample;
use crate::text::Text;

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

impl Registered for PaintingVariantValue {
    type Registry = PaintingVariantReg;
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
