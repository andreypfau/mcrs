use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;

/// A JSON value read as the NBT tree of the saved form: a number takes the first number kind that
/// holds it exactly (TXT-JSN-003), a boolean is the byte 1 or 0, an array is a list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JsonNbt(pub(crate) NbtTag);

const LONG_LIMIT: f64 = 9_223_372_036_854_775_808.0;

impl<'de> Deserialize<'de> for JsonNbt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonNbtVisitor).map(JsonNbt)
    }
}

struct JsonNbtVisitor;

impl<'de> Visitor<'de> for JsonNbtVisitor {
    type Value = NbtTag;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value other than null")
    }

    fn visit_bool<E>(self, value: bool) -> Result<NbtTag, E> {
        Ok(NbtTag::Byte(i8::from(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<NbtTag, E> {
        Ok(integer(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<NbtTag, E> {
        Ok(match i64::try_from(value) {
            Ok(value) => integer(value),
            #[expect(
                clippy::cast_precision_loss,
                reason = "a number past the long range is a double"
            )]
            Err(_) => fraction(value as f64),
        })
    }

    fn visit_f64<E>(self, value: f64) -> Result<NbtTag, E> {
        if value.fract() == 0.0 && (-LONG_LIMIT..LONG_LIMIT).contains(&value) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the value is a whole number in the long range"
            )]
            return Ok(integer(value as i64));
        }
        Ok(fraction(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<NbtTag, E> {
        Ok(NbtTag::String(value.into()))
    }

    fn visit_string<E>(self, value: String) -> Result<NbtTag, E> {
        Ok(NbtTag::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<NbtTag, A::Error> {
        let mut list: Vec<NbtTag> = Vec::with_capacity(sequence.size_hint().unwrap_or(0));
        while let Some(JsonNbt(element)) = sequence.next_element()? {
            list.push(element);
        }
        Ok(NbtTag::List(list))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NbtTag, A::Error> {
        let mut compound = NbtCompound::new();
        while let Some((key, JsonNbt(value))) = map.next_entry::<String, JsonNbt>()? {
            compound.put(&key, value);
        }
        Ok(NbtTag::Compound(compound))
    }
}

fn integer(value: i64) -> NbtTag {
    if let Ok(byte) = i8::try_from(value) {
        NbtTag::Byte(byte)
    } else if let Ok(short) = i16::try_from(value) {
        NbtTag::Short(short)
    } else if let Ok(int) = i32::try_from(value) {
        NbtTag::Int(int)
    } else {
        NbtTag::Long(value)
    }
}

fn fraction(value: f64) -> NbtTag {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the round trip tells whether a float holds the value"
    )]
    let float = value as f32;
    if f64::from(float) == value {
        NbtTag::Float(float)
    } else {
        NbtTag::Double(value)
    }
}
