use std::io::Write;

use mcrs_minecraft_registries::Wire;

use crate::item::kind::ItemComponentValue;
use crate::item::lookup::{decode_nbt_wire, encode_nbt_wire};
use crate::registry::static_registry_wire;
use crate::{Decode, Encode};
use mcrs_minecraft_item::for_each_data_component;
use mcrs_minecraft_item::keys::DataComponentType;

static_registry_wire!(DataComponentType, "data component type");

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
        impl Encode for ItemComponentValue<Wire> {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                match self {
                    $(Self::$ty(value) => if_nbt_wire!([$($flag),*] {
                        encode_nbt_wire(value, w)
                    } {
                        value.encode(w)
                    }),)*
                }
            }
        }

        /// The value of `kind`, whose wire layout the kind alone selects.
        pub fn decode_component_value(
            kind: DataComponentType,
            r: &mut &[u8],
        ) -> anyhow::Result<ItemComponentValue<Wire>> {
            Ok(match kind {
                $(DataComponentType::$kind => ItemComponentValue::$ty(if_nbt_wire!([$($flag),*] {
                    decode_nbt_wire(r)?
                } {
                    Decode::decode(r)?
                })),)*
            })
        }
    };
}

for_each_data_component!(value_wire);
