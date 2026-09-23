use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::enums::DyeColor;
use crate::item::component::text::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::item::wire::{newtype_ctx_wire, newtype_wire};
use crate::text::Text;
use crate::{Bounded, Decode, Encode};

newtype_wire!(
    CustomName,
    ItemName,
    ItemModel,
    TooltipStyle,
    NoteBlockSound
);
newtype_ctx_wire!(SignTextFront, SignTextBack);

impl Encode for Lore {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.lines.encode(w)
    }
}

impl Decode<'_> for Lore {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Lore {
            lines: Bounded::decode(r)?,
        })
    }
}

ctx_free!(Lore);

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
            <[Text; 4]>::decode_ctx(ctx, r)?,
            Option::decode_ctx(ctx, r)?,
            DyeColor::decode(r)?,
            bool::decode(r)?,
        ))
    }
}
