use mcrs_minecraft_registries::Wire;
use std::io::Write;

use crate::item::component::attribute::*;
use crate::item::wire::{newtype_wire, ordinal_enum_wire, record_wire};
use crate::{Decode, Encode, VarInt};

ordinal_enum_wire!(AttributeOperation);
newtype_wire!(AttributeModifiers<Wire>);
record_wire!(AttributeEntry<Wire> {
    attribute,
    modifier,
    slot,
    display
});
record_wire! { AttributeModifierValue { id, amount, operation } }

impl Encode for AttributeDisplay<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.type_id()).encode(&mut w)?;
        match self {
            AttributeDisplay::Override { value } => value.encode(w),
            _ => Ok(()),
        }
    }
}

/// An unknown type id reads as `Default`.
impl Decode<'_> for AttributeDisplay<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match VarInt::decode(r)?.0 {
            1 => AttributeDisplay::Hidden,
            2 => AttributeDisplay::Override {
                value: Decode::decode(r)?,
            },
            _ => AttributeDisplay::Default,
        })
    }
}
