use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::{IntoDeserializer, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{Text, TextSerde, TextTypes};

/// A translation argument: a primitive travels as itself, anything else is a
/// component, and a component that is a plain string is read as the string.
#[derive(Clone, PartialEq, Debug)]
pub enum TranslateArg<I: TextTypes> {
    Bool(bool),
    Number(NbtTag),
    Text(Text<I>),
}

impl<I: TextTypes> From<Text<I>> for TranslateArg<I> {
    fn from(text: Text<I>) -> Self {
        TranslateArg::Text(text)
    }
}

impl<I: TextTypes> From<i32> for TranslateArg<I> {
    fn from(value: i32) -> Self {
        TranslateArg::Number(NbtTag::Int(value))
    }
}

impl<I: TextTypes> From<i64> for TranslateArg<I> {
    fn from(value: i64) -> Self {
        TranslateArg::Number(NbtTag::Long(value))
    }
}

impl<I: TextTypes> From<f32> for TranslateArg<I> {
    fn from(value: f32) -> Self {
        TranslateArg::Number(NbtTag::Float(value))
    }
}

impl<I: TextTypes> From<String> for TranslateArg<I> {
    fn from(value: String) -> Self {
        TranslateArg::Text(Text::text(value))
    }
}

impl<I: TextSerde> Serialize for TranslateArg<I> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TranslateArg::Bool(v) => s.serialize_bool(*v),
            TranslateArg::Number(tag) => Serialize::serialize(tag, s),
            TranslateArg::Text(text) => text.serialize(s),
        }
    }
}

impl<'de, I: TextSerde> Deserialize<'de> for TranslateArg<I> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ArgVisitor<I>(PhantomData<I>);

        fn number<'de, I: TextTypes, E: de::Error>(
            d: impl Deserializer<'de, Error = E>,
        ) -> Result<TranslateArg<I>, E> {
            <NbtTag as Deserialize>::deserialize(d).map(TranslateArg::Number)
        }

        impl<'de, I: TextSerde> Visitor<'de> for ArgVisitor<I> {
            type Value = TranslateArg<I>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number, a boolean, a string or a text component")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(TranslateArg::Bool(v))
            }

            fn visit_i8<E: de::Error>(self, v: i8) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Byte(v)))
            }

            fn visit_i16<E: de::Error>(self, v: i16) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Short(v)))
            }

            fn visit_i32<E: de::Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Int(v)))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_f32<E: de::Error>(self, v: f32) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Float(v)))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(TranslateArg::Text(Text::text(v.to_owned())))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Text::deserialize(de::value::SeqAccessDeserializer::new(seq))
                    .map(TranslateArg::Text)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Text::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(TranslateArg::Text)
            }
        }

        d.deserialize_any(ArgVisitor(PhantomData))
    }
}
