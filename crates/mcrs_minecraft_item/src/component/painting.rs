use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_registries::{Ref, Refs, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf};
use serde::{Deserialize, Serialize};

use crate::Text;
use crate::component::common::WireRef;
use crate::harness::Sample;
use crate::harness::entry;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt};
use std::io::Write;

#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintingVariantValue<M: Refs = Resolved> {
    pub width: Bounded<1, 16, 1>,
    pub height: Bounded<1, 16, 1>,
    pub asset_id: ResourceLocationBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Text<M::Nbt>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Text<M::Nbt>>,
}

/// The persistent form is the registry id alone; the wire form carries the
/// variant inline.
#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
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

impl Encode for PaintingVariantValue<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        VarInt(self.width.0).encode(&mut w)?;
        VarInt(self.height.0).encode(&mut w)?;
        self.asset_id.encode(&mut w)?;
        self.title.encode(&mut w)?;
        self.author.encode(w)
    }
}

impl Decode<'_> for PaintingVariantValue<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(PaintingVariantValue {
            width: Bounded(VarInt::decode(r)?.0),
            height: Bounded(VarInt::decode(r)?.0),
            asset_id: ResourceLocation::decode(r)?,
            title: Option::decode(r)?,
            author: Option::decode(r)?,
        })
    }
}
