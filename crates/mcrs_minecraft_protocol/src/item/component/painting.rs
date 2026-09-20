use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_registry::RegistryLookup;
use serde::{Deserialize, Serialize};

use crate::item::component::common::{Holder, HolderWireOnly, PaintingVariantReg, Registered};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::text::Text;
use crate::{Decode, Encode, VarInt};

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

impl EncodeCtx for PaintingVariantValue {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.width.0).encode(&mut w)?;
        VarInt(self.height.0).encode(&mut w)?;
        self.asset_id.encode(&mut w)?;
        self.title.encode(&mut w)?;
        self.author.encode(w)
    }
}

impl DecodeCtx<'_> for PaintingVariantValue {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(PaintingVariantValue {
            width: Bounded(VarInt::decode(r)?.0),
            height: Bounded(VarInt::decode(r)?.0),
            asset_id: ResourceLocation::decode(r)?,
            title: Option::decode(r)?,
            author: Option::decode(r)?,
        })
    }
}

/// The persistent form is the registry id alone (`RegistryFixedCodec`); the
/// wire form carries the variant inline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaintingVariant(pub HolderWireOnly<PaintingVariantValue>);

impl EncodeCtx for PaintingVariant {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for PaintingVariant {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        HolderWireOnly::decode_ctx(ctx, r).map(PaintingVariant)
    }
}

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
