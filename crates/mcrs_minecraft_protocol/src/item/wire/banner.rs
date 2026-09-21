use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;

use crate::entity::DyeColor;
use crate::item::component::banner::*;
use crate::item::component::common::Holder;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode};

newtype_ctx_wire!(BannerPatterns);

impl EncodeCtx for BannerPattern {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.asset_id.encode(&mut w)?;
        self.translation_key.encode(w)
    }
}

impl DecodeCtx<'_> for BannerPattern {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BannerPattern {
            asset_id: ResourceLocation::decode(r)?,
            translation_key: String::decode(r)?,
        })
    }
}

impl EncodeCtx for BannerLayer {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.pattern.encode_ctx(ctx, &mut w)?;
        self.color.encode(w)
    }
}

impl DecodeCtx<'_> for BannerLayer {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BannerLayer {
            pattern: Holder::decode_ctx(ctx, r)?,
            color: DyeColor::decode(r)?,
        })
    }
}
