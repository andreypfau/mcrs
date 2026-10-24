use mcrs_minecraft_registries::Wire;
use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::Bounded;

use crate::item::component::painting::*;
use crate::item::wire::newtype_wire;
use crate::{Decode, Encode, VarInt};

newtype_wire!(PaintingVariant<Wire>);

impl Encode for PaintingVariantValue<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.width.0).encode(&mut w)?;
        VarInt(self.height.0).encode(&mut w)?;
        self.asset_id.encode(&mut w)?;
        self.title.encode(&mut w)?;
        self.author.encode(w)
    }
}

impl Decode<'_> for PaintingVariantValue<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(PaintingVariantValue {
            width: Bounded(VarInt::decode(r)?.0),
            height: Bounded(VarInt::decode(r)?.0),
            asset_id: ResourceLocation::decode(r)?,
            title: Option::decode(r)?,
            author: Option::decode(r)?,
        })
    }
}
