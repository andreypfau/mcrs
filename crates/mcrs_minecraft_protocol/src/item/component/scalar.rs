use std::io::Write;

use mcrs_minecraft_core::codec::{self, NonNegativeInt, PositiveInt, float_value, int_value};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, INT_ID};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::RgbInt;
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
    RepairCost(NonNegativeInt),
    OminousBottleAmplifier(codec::Bounded<0, 4, 0>),
}

impl Default for MaxStackSize {
    fn default() -> Self {
        MaxStackSize(codec::Bounded(64))
    }
}

macro_rules! var_int_record {
    ($($ty:ident { $field:ident: $inner:ty }),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $ty {
            pub $field: $inner,
        }

        impl Encode for $ty {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                VarInt(self.$field.0).encode(w)
            }
        }

        impl Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty { $field: codec::Bounded(VarInt::decode(r)?.0) })
            }
        }

        ctx_free!($ty);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", COMPOUND_ID), (stringify!($field), INT_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty { $field: codec::Bounded(1) },
                    $ty { $field: codec::Bounded(15) },
                    $ty { $field: codec::Bounded(i32::MAX) },
                ]
            }
        }
    )*};
}

var_int_record! {
    Enchantable { value: PositiveInt },
    VillagerFood { nutrition: PositiveInt },
}

/// `Codec.INT`: any number's `intValue()`; the wire is a VarInt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapId(#[serde(deserialize_with = "int_value")] pub i32);

impl Encode for MapId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0).encode(w)
    }
}

impl Decode<'_> for MapId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(MapId(VarInt::decode(r)?.0))
    }
}

ctx_free!(MapId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdditionalTradeCost(pub i32);

impl Encode for AdditionalTradeCost {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0).encode(w)
    }
}

impl Decode<'_> for AdditionalTradeCost {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(AdditionalTradeCost(VarInt::decode(r)?.0))
    }
}

ctx_free!(AdditionalTradeCost);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
pub struct EnchantmentGlintOverride(pub bool);

ctx_free!(EnchantmentGlintOverride);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
pub struct DyedColor(pub RgbInt);

ctx_free!(DyedColor);

/// `Codec.FLOAT.validate` over `Float.compareTo`, which orders `-0.0` below
/// `0.0` and `NaN` above everything, unlike `PartialOrd`.
fn float_in_range<E: serde::de::Error>(
    value: f32,
    min: f32,
    max: f32,
    message: impl FnOnce(f32) -> String,
) -> Result<f32, E> {
    if value.total_cmp(&min).is_lt() || value.total_cmp(&max).is_gt() {
        return Err(E::custom(message(value)));
    }
    Ok(value)
}

macro_rules! float_newtype {
    ($($(#[$meta:meta])* $ty:ident [$min:expr, $max:expr] $message:expr),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Default, Encode, Decode)]
        pub struct $ty(pub f32);

        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_f32(self.0)
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = float_value(d)?;
                float_in_range::<D::Error>(value, $min, $max, $message).map($ty)
            }
        }

        ctx_free!($ty);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", FLOAT_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![$ty(0.0), $ty(0.5), $ty(1.0)]
            }
        }
    )*};
}

float_newtype! {
    /// `ExtraCodecs.floatRange(0, 1)`.
    MinimumAttackCharge [0.0, 1.0] |n| format!("Value must be within range [0.0;1.0]: {n:?}"),
    /// `ExtraCodecs.NON_NEGATIVE_FLOAT`.
    PotionDurationScale [0.0, f32::MAX] |n| format!("Value must be non-negative: {n:?}"),
}

impl Sample for MaxStackSize {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", INT_ID)]
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
        vec![("", INT_ID)]
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
        vec![("", INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![Damage(codec::Bounded(0)), Damage(codec::Bounded(300))]
    }
}

impl Sample for RepairCost {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            RepairCost(codec::Bounded(0)),
            RepairCost(codec::Bounded(5)),
            RepairCost(codec::Bounded(i32::MAX)),
        ]
    }
}

impl Sample for OminousBottleAmplifier {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            OminousBottleAmplifier(codec::Bounded(0)),
            OminousBottleAmplifier(codec::Bounded(4)),
        ]
    }
}

impl Sample for MapId {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![MapId(0), MapId(12345), MapId(-1)]
    }
}

impl Sample for AdditionalTradeCost {
    fn samples() -> Vec<Self> {
        vec![
            AdditionalTradeCost(0),
            AdditionalTradeCost(300),
            AdditionalTradeCost(-1),
        ]
    }
}

impl Sample for EnchantmentGlintOverride {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", BYTE_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            EnchantmentGlintOverride(true),
            EnchantmentGlintOverride(false),
        ]
    }
}

impl Sample for DyedColor {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", INT_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            DyedColor(RgbInt(0)),
            DyedColor(RgbInt(0xFF0000)),
            DyedColor(RgbInt(-6265536)),
        ]
    }
}
