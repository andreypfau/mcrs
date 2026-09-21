use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec;

use crate::item::component::scalar::*;
use crate::item::wire::{bounded_var_int_wire, newtype_wire, var_int_wire};
use crate::{Decode, Encode, VarInt};

bounded_var_int_wire!(MaxStackSize, MaxDamage, Damage, RepairCost, OminousBottleAmplifier);
var_int_wire!(MapId, AdditionalTradeCost);
newtype_wire!(
    EnchantmentGlintOverride,
    DyedColor,
    MinimumAttackCharge,
    PotionDurationScale,
);

macro_rules! var_int_record_wire {
    ($($ty:ident { $field:ident } $(=> $guard:expr)?),* $(,)?) => {$(
        impl Encode for $ty {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(self.$field.0).encode(w)
            }
        }

        impl Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let $field = VarInt::decode(r)?.0;
                $($guard;)?
                Ok($ty { $field: codec::Bounded($field) })
            }
        }

        crate::item::ctx::ctx_free!($ty);
    )*};
}

var_int_record_wire! {
    Enchantable { value }
        => ensure!(value > 0, "Enchantment value must be positive, but was {value}"),
    VillagerFood { nutrition },
}
