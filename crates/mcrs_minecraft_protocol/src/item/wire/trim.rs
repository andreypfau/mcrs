use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::common::Holder;
use crate::item::component::trim::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::text::Text;
use crate::{Decode, Encode};

impl EncodeCtx for TrimMaterial {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.palette_id.encode(&mut w)?;
        self.description.encode(w)
    }
}

impl DecodeCtx<'_> for TrimMaterial {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrimMaterial {
            palette_id: ResourceLocation::decode(r)?,
            description: Text::decode(r)?,
        })
    }
}

impl EncodeCtx for TrimPattern {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.asset_id.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.decal.encode(w)
    }
}

impl DecodeCtx<'_> for TrimPattern {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(TrimPattern {
            asset_id: ResourceLocation::decode(r)?,
            description: Text::decode(r)?,
            decal: bool::decode(r)?,
        })
    }
}

impl EncodeCtx for Trim {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.material.encode_ctx(ctx, &mut w)?;
        self.pattern.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Trim {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Trim {
            material: Holder::decode_ctx(ctx, r)?,
            pattern: Holder::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for ProvidesTrimMaterial {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for ProvidesTrimMaterial {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(ProvidesTrimMaterial)
    }
}
