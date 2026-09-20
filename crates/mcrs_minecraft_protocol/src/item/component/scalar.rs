use std::io::Write;

use mcrs_minecraft_core::codec::{self, NonNegativeInt, PositiveInt};
use serde::{Deserialize, Serialize};

use crate::item::component::common::stub_component;
use crate::item::ctx::ctx_free;
use crate::item::harness::Sample;
use crate::{Decode, Encode, VarInt};

macro_rules! var_int_newtype {
    ($($(#[$meta:meta])* $ty:ident($inner:ty)),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub $inner);

        impl Encode for $ty {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(self.0.0).encode(w)
            }
        }

        impl Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty(codec::Bounded(VarInt::decode(r)?.0)))
            }
        }

        ctx_free!($ty);
    )*};
}

var_int_newtype! {
    #[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
    MaxStackSize(codec::Bounded<1, 99, 64>),
    MaxDamage(PositiveInt),
    Damage(NonNegativeInt),
}

impl Default for MaxStackSize {
    fn default() -> Self {
        MaxStackSize(codec::Bounded(64))
    }
}

impl Sample for MaxStackSize {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            MaxStackSize::default(),
            MaxStackSize(codec::Bounded(1)),
            MaxStackSize(codec::Bounded(99)),
        ]
    }
}

impl Sample for MaxDamage {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            MaxDamage(codec::Bounded(1)),
            MaxDamage(codec::Bounded(1561)),
            MaxDamage(codec::Bounded(i32::MAX)),
        ]
    }
}

impl Sample for Damage {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![Damage(codec::Bounded(0)), Damage(codec::Bounded(300))]
    }
}

stub_component!(
    MinimumAttackCharge,
    RepairCost,
    EnchantmentGlintOverride,
    Enchantable,
    AdditionalTradeCost,
    VillagerFood,
    DyedColor,
    MapId,
    PotionDurationScale,
    OminousBottleAmplifier,
);
