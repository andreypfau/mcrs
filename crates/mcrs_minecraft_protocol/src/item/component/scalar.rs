use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec::{self, NonNegativeInt, PositiveInt, float_value, int_value};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, INT_ID};
use serde::de::Error as _;
use serde::ser::Error as _;
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

/// A record codec reads a map and nothing else, where the derived visitor
/// would also take the fields as a sequence.
macro_rules! record_codec {
    ($($ty:ident),* $(,)?) => {$(
        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct MapOnly;

                impl<'de> serde::de::Visitor<'de> for MapOnly {
                    type Value = $ty;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a map")
                    }

                    fn visit_map<A: serde::de::MapAccess<'de>>(
                        self,
                        map: A,
                    ) -> Result<$ty, A::Error> {
                        $ty::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    }
                }

                d.deserialize_map(MapOnly)
            }
        }

        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                $ty::serialize(self, s)
            }
        }
    )*};
}
pub(crate) use record_codec;

macro_rules! var_int_record {
    ($($ty:ident { $field:ident: $inner:ty } $(=> $guard:expr)?),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub struct $ty {
            pub $field: $inner,
        }

        record_codec!($ty);

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
    Enchantable { value: PositiveInt }
        => ensure!(value > 0, "Enchantment value must be positive, but was {value}"),
    VillagerFood { nutrition: PositiveInt },
}

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

/// Bounds order `-0.0` below `0.0` and `NaN` above everything, unlike
/// `PartialOrd`.
fn float_in_range(
    value: f32,
    min: f32,
    max: f32,
    message: impl FnOnce(f32) -> String,
) -> Result<f32, String> {
    if value.total_cmp(&min).is_lt() || value.total_cmp(&max).is_gt() {
        return Err(message(value));
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
                let value = float_in_range(self.0, $min, $max, $message).map_err(S::Error::custom)?;
                s.serialize_f32(value)
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = float_value(d)?;
                float_in_range(value, $min, $max, $message)
                    .map($ty)
                    .map_err(D::Error::custom)
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
    MinimumAttackCharge [0.0, 1.0] |n| format!("Value must be within range [0.0;1.0]: {n:?}"),
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
