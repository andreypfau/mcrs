use std::io::Write;

use crate::item::ctx::WireCtx;

use crate::item::component::attribute::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::{newtype_ctx_wire, ordinal_enum_wire, record_ctx_wire, record_wire};
use crate::{Decode, Encode, VarInt};

ordinal_enum_wire!(AttributeOperation);
newtype_ctx_wire!(AttributeModifiers);
record_ctx_wire!(AttributeEntry {
    attribute,
    modifier,
    slot,
    display
});
record_wire! { AttributeModifierValue { id, amount, operation } }

impl EncodeCtx for AttributeDisplay {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.type_id()).encode(&mut w)?;
        match self {
            AttributeDisplay::Override { value } => value.encode_ctx(ctx, w),
            _ => Ok(()),
        }
    }
}

/// An unknown type id reads as `Default`.
impl DecodeCtx<'_> for AttributeDisplay {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override {
                value: DecodeCtx::decode_ctx(ctx, r)?,
            },
            _ => AttributeDisplay::Default,
        })
    }
}
