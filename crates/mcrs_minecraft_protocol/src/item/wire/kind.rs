use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire, scoped};
use crate::item::kind::ItemComponentValue;
use crate::registry::static_registry_wire;
use mcrs_minecraft_item::for_each_data_component;
use mcrs_minecraft_item::keys::DataComponentType;

static_registry_wire!(DataComponentType, "data component type");
crate::item::ctx::ctx_free!(DataComponentType);

macro_rules! value_wire {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        impl EncodeCtx for ItemComponentValue {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                scoped(ctx, || match self {
                    $(Self::$ty(value) => {
                        if DataComponentType::$kind.is_nbt_wire() {
                            encode_nbt_wire(value, w)
                        } else {
                            value.encode_ctx(ctx, w)
                        }
                    })*
                })
            }
        }

        /// The value of `kind`, whose wire layout the kind alone selects.
        pub fn decode_component_value(
            kind: DataComponentType,
            ctx: &dyn RegistryLookup,
            r: &mut &[u8],
        ) -> anyhow::Result<ItemComponentValue> {
            scoped(ctx, || match kind {
                $(DataComponentType::$kind => Ok(ItemComponentValue::$ty(if kind.is_nbt_wire() {
                    decode_nbt_wire(r)?
                } else {
                    <$ty>::decode_ctx(ctx, r)?
                })),)*
            })
        }
    };
}

for_each_data_component!(value_wire);
