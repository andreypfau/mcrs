use mcrs_minecraft_core::codec::{self, NonNegativeInt, PositiveInt, float_value, int_value};
use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, FLOAT_ID, INT_ID};
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::component::common::RgbInt;
use crate::harness::Sample;

macro_rules! var_int_newtype {
    ($($(#[$meta:meta])* $ty:ident($(#[$field:meta])* $inner:ty) [$($sample:expr),+ $(,)?]),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty($(#[$field])* pub $inner);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", INT_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![$($ty($sample)),+]
            }
        }
    )*};
}

var_int_newtype! {
    #[derive(Default)]
    MaxStackSize(codec::Bounded<1, 99, 64>) [codec::Bounded(64), codec::Bounded(1), codec::Bounded(99)],
    MaxDamage(PositiveInt) [codec::Bounded(1), codec::Bounded(1561), codec::Bounded(i32::MAX)],
    Damage(NonNegativeInt) [codec::Bounded(0), codec::Bounded(300)],
    RepairCost(NonNegativeInt) [codec::Bounded(0), codec::Bounded(5), codec::Bounded(i32::MAX)],
    OminousBottleAmplifier(codec::Bounded<0, 4, 0>) [codec::Bounded(0), codec::Bounded(4)],
    #[derive(Default)]
    MapId(#[serde(deserialize_with = "int_value")] i32) [0, 12345, -1],
    #[derive(Default)]
    DyedColor(RgbInt) [RgbInt(0), RgbInt(0xFF0000), RgbInt(-6265536)],
}

/// A record codec reads a map and nothing else, where the derived visitor
/// would also take the fields as a sequence.
macro_rules! record_codec {
    ($($ty:ident $(<$param:ident>)?),* $(,)?) => {$(
        impl<'de $(, $param: serde::Deserialize<'de>)?> serde::Deserialize<'de> for $ty $(<$param>)? {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct MapOnly<T>(std::marker::PhantomData<T>);

                impl<'de $(, $param: serde::Deserialize<'de>)?> serde::de::Visitor<'de>
                    for MapOnly<$ty $(<$param>)?>
                {
                    type Value = $ty $(<$param>)?;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a map")
                    }

                    fn visit_map<A: serde::de::MapAccess<'de>>(
                        self,
                        map: A,
                    ) -> Result<Self::Value, A::Error> {
                        <$ty $(<$param>)?>::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    }
                }

                d.deserialize_map(MapOnly(std::marker::PhantomData))
            }
        }

        impl $(<$param: serde::Serialize>)? serde::Serialize for $ty $(<$param>)? {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                <$ty $(<$param>)?>::serialize(self, s)
            }
        }
    )*};
}
pub(crate) use record_codec;

macro_rules! var_int_record {
    ($($ty:ident { $field:ident: $inner:ty }),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub struct $ty {
            pub $field: $inner,
        }

        record_codec!($ty);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdditionalTradeCost(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnchantmentGlintOverride(pub bool);

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
        #[derive(Clone, Copy, Debug, PartialEq, Default)]
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
