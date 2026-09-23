use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::attribute::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::{newtype_ctx_wire, ordinal_enum_wire, record_ctx_wire, record_wire};
use crate::text::Text;
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
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.type_id()).encode(&mut w)?;
        match self {
            AttributeDisplay::Override { value } => value.encode(w),
            _ => Ok(()),
        }
    }
}

/// An unknown type id reads as `Default`.
impl DecodeCtx<'_> for AttributeDisplay {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override {
                value: Text::decode(r)?,
            },
            _ => AttributeDisplay::Default,
        })
    }
}
