use std::io::Write;

use anyhow::bail;
use mcrs_minecraft_registry::RegistryLookup;

use crate::for_each_data_component;
use crate::item::component::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire};
use crate::item::kind::{ItemComponentKind, ItemComponentValue};
use crate::{Decode, Encode, VarInt};

impl Encode for ItemComponentKind {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.wire_id() as i32).encode(w)
    }
}

impl Decode<'_> for ItemComponentKind {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        match u16::try_from(id).ok().and_then(Self::from_wire_id) {
            Some(kind) => Ok(kind),
            None => bail!("unknown data component type {id}"),
        }
    }
}

crate::item::ctx::ctx_free!(ItemComponentKind);

macro_rules! value_wire {
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        impl EncodeCtx for ItemComponentValue {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                match self {
                    $(Self::$ty(value) => {
                        if ItemComponentKind::$ty.is_nbt_wire() {
                            encode_nbt_wire(value, w)
                        } else {
                            value.encode_ctx(ctx, w)
                        }
                    })*
                }
            }
        }

        /// The value of `kind`, whose wire layout the kind alone selects.
        pub fn decode_component_value(
            kind: ItemComponentKind,
            ctx: &dyn RegistryLookup,
            r: &mut &[u8],
        ) -> anyhow::Result<ItemComponentValue> {
            match kind {
                $(ItemComponentKind::$ty => Ok(ItemComponentValue::$ty(if kind.is_nbt_wire() {
                    decode_nbt_wire(r)?
                } else {
                    <$ty>::decode_ctx(ctx, r)?
                })),)*
            }
        }
    };
}

for_each_data_component!(value_wire);
