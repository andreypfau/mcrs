use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::enums::DyeColor;
use crate::item::component::text::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::{newtype_ctx_wire, newtype_wire};
use crate::{Decode, Encode};

newtype_wire!(ItemModel, TooltipStyle, NoteBlockSound);
newtype_ctx_wire!(CustomName, ItemName, SignTextFront, SignTextBack);

impl EncodeCtx for Lore {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.lines.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Lore {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Lore {
            lines: DecodeCtx::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for SignText {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.messages.encode_ctx(ctx, &mut w)?;
        self.filtered_messages.encode_ctx(ctx, &mut w)?;
        self.color.encode(&mut w)?;
        self.has_glowing_text.encode(w)
    }
}

impl DecodeCtx<'_> for SignText {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SignText::new(
            DecodeCtx::decode_ctx(ctx, r)?,
            Option::decode_ctx(ctx, r)?,
            DyeColor::decode(r)?,
            bool::decode(r)?,
        ))
    }
}
