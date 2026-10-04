use serde::de::value::{MapDeserializer, SeqDeserializer};
use serde::de::{self, DeserializeSeed, IntoDeserializer, Visitor};
use serde::{Deserialize, forward_to_deserialize_any};
use std::convert::identity;

use crate::tag::NbtTag;
use crate::{Error, NBT_ARRAY_TAG, NBT_BYTE_ARRAY_TAG, NBT_INT_ARRAY_TAG, NBT_LONG_ARRAY_TAG};

pub type Result<T> = std::result::Result<T, Error>;

pub fn from_tag<'de, T: Deserialize<'de>>(tag: NbtTag) -> Result<T> {
    let _binary = crate::BinaryReadGuard::new();
    T::deserialize(tag)
}

impl<'de> IntoDeserializer<'de, Error> for NbtTag {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

/// Integer tags keep their low bits; float tags floor and saturate, since
/// the game's number tags do not wrap a float.
macro_rules! numeric_read {
    ($name:ident, $visit:ident, $target:ty, $float:path, $double:path) => {
        fn $name<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self {
                NbtTag::Byte(v) => visitor.$visit(v as $target),
                NbtTag::Short(v) => visitor.$visit(v as $target),
                NbtTag::Int(v) => visitor.$visit(v as $target),
                NbtTag::Long(v) => visitor.$visit(v as $target),
                NbtTag::Float(v) => visitor.$visit($float(v) as $target),
                NbtTag::Double(v) => visitor.$visit($double(v) as $target),
                other => other.deserialize_any(visitor),
            }
        }
    };
}

impl<'de> de::Deserializer<'de> for NbtTag {
    type Error = Error;

    forward_to_deserialize_any! {
        char str string seq tuple tuple_struct map struct identifier unit_struct
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::End => Err(Error::SerdeError(
                "cannot deserialize a value from TAG_End".to_string(),
            )),
            NbtTag::Byte(v) => visitor.visit_i8(v),
            NbtTag::Short(v) => visitor.visit_i16(v),
            NbtTag::Int(v) => visitor.visit_i32(v),
            NbtTag::Long(v) => visitor.visit_i64(v),
            NbtTag::Float(v) => visitor.visit_f32(v),
            NbtTag::Double(v) => visitor.visit_f64(v),
            NbtTag::String(v) => visitor.visit_string(v),
            NbtTag::ByteArray(v) => visitor.visit_seq(SeqDeserializer::new(
                v.into_vec().into_iter().map(|b| NbtTag::Byte(b as i8)),
            )),
            NbtTag::IntArray(v) => {
                visitor.visit_seq(SeqDeserializer::new(v.into_iter().map(NbtTag::Int)))
            }
            NbtTag::LongArray(v) => {
                visitor.visit_seq(SeqDeserializer::new(v.into_iter().map(NbtTag::Long)))
            }
            NbtTag::List(v) => visitor.visit_seq(SeqDeserializer::new(v.into_iter())),
            NbtTag::Compound(v) => {
                visitor.visit_map(MapDeserializer::new(v.child_tags.into_iter()))
            }
        }
    }

    /// Any numeric tag, true when non-zero.
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::Byte(v) => visitor.visit_bool(v != 0),
            NbtTag::Short(v) => visitor.visit_bool(v != 0),
            NbtTag::Int(v) => visitor.visit_bool(v != 0),
            NbtTag::Long(v) => visitor.visit_bool(v != 0),
            NbtTag::Float(v) => visitor.visit_bool(v != 0.0),
            NbtTag::Double(v) => visitor.visit_bool(v != 0.0),
            other => other.deserialize_any(visitor),
        }
    }

    numeric_read!(deserialize_i8, visit_i8, i8, f32::floor, f64::floor);
    numeric_read!(deserialize_i16, visit_i16, i16, f32::floor, f64::floor);
    numeric_read!(deserialize_i32, visit_i32, i32, f32::floor, f64::floor);
    numeric_read!(deserialize_i64, visit_i64, i64, f32::floor, f64::floor);
    numeric_read!(deserialize_f32, visit_f32, f32, identity, identity);
    numeric_read!(deserialize_f64, visit_f64, f64, identity, identity);

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::Byte(v) => visitor.visit_u8(v as u8),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::Short(v) => visitor.visit_u16(v as u16),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::Int(v) => visitor.visit_u32(v as u32),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::Long(v) => visitor.visit_u64(v as u64),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::ByteArray(v) => visitor.visit_bytes(&v),
            NbtTag::IntArray(v) => {
                visitor.visit_byte_buf(v.into_iter().flat_map(i32::to_be_bytes).collect())
            }
            NbtTag::LongArray(v) => {
                visitor.visit_byte_buf(v.into_iter().flat_map(i64::to_be_bytes).collect())
            }
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::End => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            NbtTag::End => visitor.visit_unit(),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        let variant = match (name, &self) {
            (NBT_ARRAY_TAG, NbtTag::ByteArray(_)) => NBT_BYTE_ARRAY_TAG,
            (NBT_ARRAY_TAG, NbtTag::IntArray(_)) => NBT_INT_ARRAY_TAG,
            (NBT_ARRAY_TAG, NbtTag::LongArray(_)) => NBT_LONG_ARRAY_TAG,
            _ => return visitor.visit_newtype_struct(self),
        };
        visitor.visit_enum(ArrayAccess { tag: self, variant })
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self {
            NbtTag::String(variant) => visitor.visit_enum(variant.into_deserializer()),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct ArrayAccess {
    tag: NbtTag,
    variant: &'static str,
}

impl<'de> de::EnumAccess<'de> for ArrayAccess {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de> de::VariantAccess<'de> for ArrayAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::UnsupportedType("array as unit variant".to_string()))
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.tag)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value> {
        Err(Error::UnsupportedType("array as tuple variant".to_string()))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value> {
        Err(Error::UnsupportedType(
            "array as struct variant".to_string(),
        ))
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use super::from_tag;
    use crate::compound::NbtCompound;
    use crate::tag::NbtTag;
    use crate::{nbt_byte_array, nbt_int_array, nbt_long_array, to_nbt_compound};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Value {
        byte: i8,
        flag: bool,
        text: String,
        #[serde(serialize_with = "nbt_int_array")]
        ints: Vec<i32>,
        #[serde(serialize_with = "nbt_long_array")]
        longs: Vec<i64>,
        #[serde(serialize_with = "nbt_byte_array")]
        bytes: Vec<u8>,
        list: Vec<f32>,
        nested: Option<Box<Value>>,
        missing: Option<i32>,
        tag: NbtTag,
    }

    fn sample() -> Value {
        Value {
            byte: -3,
            flag: true,
            text: "hi".to_string(),
            ints: vec![1, -2],
            longs: vec![i64::MIN],
            bytes: vec![0, 255],
            list: vec![1.5, -2.0],
            nested: Some(Box::new(Value {
                byte: 1,
                flag: false,
                text: String::new(),
                ints: vec![],
                longs: vec![],
                bytes: vec![],
                list: vec![],
                nested: None,
                missing: Some(7),
                tag: NbtTag::List(vec![NbtTag::String("a".into()), NbtTag::Int(1)]),
            })),
            missing: None,
            tag: NbtTag::Compound(NbtCompound::new()),
        }
    }

    #[test]
    fn a_struct_round_trips_through_memory() {
        let value = sample();
        let compound = to_nbt_compound(&value).unwrap();
        assert_eq!(compound.get("ints"), Some(&NbtTag::IntArray(vec![1, -2])));
        let read: Value = from_tag(NbtTag::Compound(compound)).unwrap();
        assert_eq!(read, value);
    }

    #[test]
    fn a_tag_reads_back_as_itself() {
        let tag = NbtTag::Compound(to_nbt_compound(&sample()).unwrap());
        assert_eq!(from_tag::<NbtTag>(tag.clone()).unwrap(), tag);
        let list = NbtTag::List(vec![
            NbtTag::ByteArray(Box::new([1, 2])),
            NbtTag::LongArray(vec![3]),
        ]);
        assert_eq!(from_tag::<NbtTag>(list.clone()).unwrap(), list);
    }

    #[test]
    fn an_end_tag_is_none_and_a_mismatch_is_an_error() {
        assert_eq!(from_tag::<Option<i32>>(NbtTag::End).unwrap(), None);
        assert_eq!(from_tag::<Option<i32>>(NbtTag::Int(4)).unwrap(), Some(4));
        assert!(from_tag::<i32>(NbtTag::String("4".into())).is_err());
        assert!(from_tag::<Value>(NbtTag::Int(4)).is_err());
    }

    #[test]
    fn an_unsigned_read_keeps_the_bits_of_its_own_tag() {
        assert_eq!(from_tag::<u8>(NbtTag::Byte(-1)).unwrap(), 255);
        assert_eq!(from_tag::<u16>(NbtTag::Short(-1)).unwrap(), u16::MAX);
        assert_eq!(from_tag::<u32>(NbtTag::Int(-1)).unwrap(), u32::MAX);
        assert_eq!(from_tag::<u64>(NbtTag::Long(-1)).unwrap(), u64::MAX);
        assert!(from_tag::<u32>(NbtTag::Double(1.0)).is_err());
    }

    #[test]
    fn a_non_numeric_tag_is_no_number_of_any_type() {
        let text = || NbtTag::String("4".into());
        assert!(from_tag::<i8>(text()).is_err());
        assert!(from_tag::<i16>(text()).is_err());
        assert!(from_tag::<i32>(text()).is_err());
        assert!(from_tag::<i64>(text()).is_err());
        assert!(from_tag::<f32>(text()).is_err());
        assert!(from_tag::<f64>(text()).is_err());
        assert!(from_tag::<i32>(NbtTag::List(vec![NbtTag::Int(1)])).is_err());
        assert!(from_tag::<i64>(NbtTag::Compound(NbtCompound::new())).is_err());
    }

    #[test]
    fn an_int_is_read_from_every_numeric_tag() {
        let int = |tag| from_tag::<i32>(tag).unwrap();
        assert_eq!(int(NbtTag::Byte(-7)), -7);
        assert_eq!(int(NbtTag::Short(-300)), -300);
        assert_eq!(int(NbtTag::Int(i32::MIN)), i32::MIN);
        assert_eq!(int(NbtTag::Long((1 << 32) + 5)), 5);
        assert_eq!(int(NbtTag::Long(1 << 31)), i32::MIN);
        assert_eq!(int(NbtTag::Long(-1)), -1);
        assert_eq!(int(NbtTag::Double(2.9)), 2);
        assert_eq!(int(NbtTag::Double(-0.5)), -1);
        assert_eq!(int(NbtTag::Double(1e10)), i32::MAX);
        assert_eq!(int(NbtTag::Double(-1e10)), i32::MIN);
        assert_eq!(int(NbtTag::Double(f64::NAN)), 0);
        assert_eq!(int(NbtTag::Double(f64::INFINITY)), i32::MAX);
        assert_eq!(int(NbtTag::Double(f64::NEG_INFINITY)), i32::MIN);
        assert_eq!(int(NbtTag::Float(2.9)), 2);
        assert_eq!(int(NbtTag::Float(-0.5)), -1);
        assert_eq!(int(NbtTag::Float(3e9)), i32::MAX);
        assert_eq!(int(NbtTag::Float(-3e9)), i32::MIN);
        assert_eq!(int(NbtTag::Float(f32::NAN)), 0);
        assert_eq!(int(NbtTag::Float(f32::INFINITY)), i32::MAX);
        assert_eq!(int(NbtTag::Float(f32::NEG_INFINITY)), i32::MIN);
    }
}
