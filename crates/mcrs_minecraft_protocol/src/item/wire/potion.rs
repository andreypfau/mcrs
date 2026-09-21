use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::common::MobEffectInstance;
use crate::item::component::potion::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::{Decode, Encode};

impl EncodeCtx for PotionContents {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.potion.encode_ctx(ctx, &mut w)?;
        self.custom_color.encode(&mut w)?;
        self.visible_effects().encode_ctx(ctx, &mut w)?;
        self.custom_name.encode(w)
    }
}

impl DecodeCtx<'_> for PotionContents {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let potion = Option::decode_ctx(ctx, r)?;
        let custom_color = Option::decode(r)?;
        let mut custom_effects: Vec<MobEffectInstance> = Vec::decode_ctx(ctx, r)?;
        strip_hidden(&mut custom_effects);
        Ok(PotionContents {
            potion,
            custom_color,
            custom_effects,
            custom_name: Option::decode(r)?,
        })
    }
}
