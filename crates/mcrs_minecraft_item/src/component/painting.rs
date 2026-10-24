use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::WireRef;
use crate::harness::Sample;
use crate::harness::entry;

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintingVariantValue<M: Refs = Resolved> {
    pub width: Bounded<1, 16, 1>,
    pub height: Bounded<1, 16, 1>,
    pub asset_id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Text<M::Nbt>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Text<M::Nbt>>,
}

/// The persistent form is the registry id alone; the wire form carries the
/// variant inline.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaintingVariant<M: Refs = Resolved>(
    pub WireRef<M, PaintingVariantValue, PaintingVariantValue<M>>,
);

impl Sample for PaintingVariant {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::STRING_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![PaintingVariant(WireRef(Ref::Id(entry("kebab"))))]
    }
}
