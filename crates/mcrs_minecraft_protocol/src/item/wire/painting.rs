use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::painting::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(PaintingVariant);

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
