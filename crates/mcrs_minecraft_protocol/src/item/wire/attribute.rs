use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::attribute::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
use crate::item::wire::{newtype_ctx_wire, ordinal_enum_wire, record_ctx_wire};
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

impl Encode for AttributeModifierValue {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode(&mut w)?;
        self.amount.encode(&mut w)?;
        self.operation.encode(w)
    }
}

impl Decode<'_> for AttributeModifierValue {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(AttributeModifierValue {
            id: ResourceLocation::decode(r)?,
            amount: f64::decode(r)?,
            operation: AttributeOperation::decode(r)?,
        })
    }
}

ctx_free!(AttributeModifierValue);

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
