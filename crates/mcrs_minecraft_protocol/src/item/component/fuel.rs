use std::fmt;
use std::io::Write;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::{COMPOUND_ID, FLOAT_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::item::component::common::RegistryName;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::{Decode, Encode};

pub enum NumberProviderReg {}

impl RegistryName for NumberProviderReg {
    const NAME: &'static str = "number_provider";
}

/// A float, or the id of a number provider; the wire is a flag then the float
/// or the id string.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvableNumber {
    Constant(f32),
    Reference(ResourceKey<NumberProviderReg>),
}

impl ResolvableNumber {
    pub fn reference(location: ResourceLocation) -> Self {
        ResolvableNumber::Reference(ResourceKey::from_location(location))
    }
}

impl Serialize for ResolvableNumber {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ResolvableNumber::Constant(value) => s.serialize_f32(*value),
            ResolvableNumber::Reference(key) => key.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for ResolvableNumber {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct NumberVisitor;

        impl Visitor<'_> for NumberVisitor {
            type Value = ResolvableNumber;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a number provider id")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                ResourceLocation::read(text)
                    .map(ResolvableNumber::reference)
                    .map_err(E::custom)
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(ResolvableNumber::Constant(v as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ResolvableNumber::Constant(v as f32))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(ResolvableNumber::Constant(v as f32))
            }
        }

        d.deserialize_any(NumberVisitor)
    }
}

impl EncodeCtx for ResolvableNumber {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            ResolvableNumber::Constant(value) => {
                true.encode(&mut w)?;
                value.encode(w)
            }
            ResolvableNumber::Reference(key) => {
                false.encode(&mut w)?;
                key.location().encode(w)
            }
        }
    }
}

impl DecodeCtx<'_> for ResolvableNumber {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => ResolvableNumber::Constant(f32::decode(r)?),
            false => ResolvableNumber::reference(ResourceLocation::decode(r)?),
        })
    }
}

fn tag_of(number: &ResolvableNumber) -> u8 {
    match number {
        ResolvableNumber::Constant(_) => FLOAT_ID,
        ResolvableNumber::Reference(_) => STRING_ID,
    }
}

macro_rules! fuel {
    ($($ty:ident { $($field:ident),+ }),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $ty {
            $(pub $field: ResolvableNumber,)+
        }

        impl EncodeCtx for $ty {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
                $(self.$field.encode_ctx(ctx, &mut w)?;)+
                Ok(())
            }
        }

        impl DecodeCtx<'_> for $ty {
            fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty {
                    $($field: ResolvableNumber::decode_ctx(ctx, r)?,)+
                })
            }
        }

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", COMPOUND_ID), $((stringify!($field), tag_of(&self.$field)),)+]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty { $($field: ResolvableNumber::Constant(3.0),)+ },
                    $ty { $($field: ResolvableNumber::reference(ResourceLocation::minecraft(stringify!($field))),)+ },
                ]
            }
        }
    )*};
}

fuel! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
