use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::fireworks::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::item::wire::ordinal_enum_wire;
use crate::{Bounded, Decode, Encode, VarInt};

ordinal_enum_wire!(FireworkShape);

impl Encode for FireworkExplosion {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.shape.encode(&mut w)?;
        self.colors.encode(&mut w)?;
        self.fade_colors.encode(&mut w)?;
        self.has_trail.encode(&mut w)?;
        self.has_twinkle.encode(w)
    }
}

impl Decode<'_> for FireworkExplosion {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(FireworkExplosion {
            shape: FireworkShape::decode(r)?,
            colors: Vec::decode(r)?,
            fade_colors: Vec::decode(r)?,
            has_trail: bool::decode(r)?,
            has_twinkle: bool::decode(r)?,
        })
    }
}

ctx_free!(FireworkExplosion);

impl EncodeCtx for Fireworks {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.flight_duration).encode(&mut w)?;
        self.explosions.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Fireworks {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Fireworks {
            flight_duration: VarInt::decode(r)?.0,
            explosions: Bounded::decode_ctx(ctx, r)?,
        })
    }
}
