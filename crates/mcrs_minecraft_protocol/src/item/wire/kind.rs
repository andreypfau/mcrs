use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::*;
use crate::item::ctx::{
    DecodeCtx, EncodeCtx, decode_named_nbt, decode_nbt_wire, encode_named_nbt, encode_nbt_wire,
};
use crate::item::kind::ItemComponentValue;
use crate::registry::static_registry_wire;
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_item::{for_each_data_component, if_refs};

static_registry_wire!(DataComponentType, "data component type");
crate::item::ctx::ctx_free!(DataComponentType);

/// Expands to the first group when the flag list holds `nbt_wire`, else to the second.
macro_rules! if_nbt_wire {
    ([] $then:tt {$($else:tt)*}) => { $($else)* };
    ([nbt_wire $(, $rest:ident)*] {$($then:tt)*} $else:tt) => { $($then)* };
    ([$first:ident $(, $rest:ident)*] $then:tt $else:tt) => {
        if_nbt_wire!([$($rest),*] $then $else)
    };
}

macro_rules! value_wire {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        impl EncodeCtx for ItemComponentValue {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                match self {
                    $(Self::$ty(value) => if_nbt_wire!([$($flag),*] {
                        if_refs!([$($flag),*] {
                            encode_named_nbt(value, ctx, w)
                        } {
                            encode_nbt_wire(value, w)
                        })
                    } {
                        value.encode_ctx(ctx, w)
                    }),)*
                }
            }
        }

        /// The value of `kind`, whose wire layout the kind alone selects.
        pub fn decode_component_value(
            kind: DataComponentType,
            ctx: &dyn RegistryLookup,
            r: &mut &[u8],
        ) -> anyhow::Result<ItemComponentValue> {
            Ok(match kind {
                $(DataComponentType::$kind => ItemComponentValue::$ty(if_nbt_wire!([$($flag),*] {
                    if_refs!([$($flag),*] {
                        decode_named_nbt::<$ty>(ctx, r)?
                    } {
                        decode_nbt_wire::<$ty>(r)?
                    })
                } {
                    <$ty>::decode_ctx(ctx, r)?
                })),)*
            })
        }
    };
}

for_each_data_component!(value_wire);
