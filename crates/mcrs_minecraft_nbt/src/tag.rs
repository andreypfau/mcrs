use compound::NbtCompound;
use deserializer::NbtReadHelper;
use io::{Read, Write};
use serde::{Deserialize, Serialize};
use serializer::WriteAdaptor;

use crate::*;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum NbtTag {
    End = END_ID,
    Byte(i8) = BYTE_ID,
    Short(i16) = SHORT_ID,
    Int(i32) = INT_ID,
    Long(i64) = LONG_ID,
    Float(f32) = FLOAT_ID,
    Double(f64) = DOUBLE_ID,
    ByteArray(Box<[u8]>) = BYTE_ARRAY_ID,
    String(String) = STRING_ID,
    List(Vec<NbtTag>) = LIST_ID,
    Compound(NbtCompound) = COMPOUND_ID,
    IntArray(Vec<i32>) = INT_ARRAY_ID,
    LongArray(Vec<i64>) = LONG_ARRAY_ID,
}

impl NbtTag {
    /// Returns the numeric id associated with the data type.
    pub const fn get_type_id(&self) -> u8 {
        // Safety: Since Self is repr(u8), it is guaranteed to hold the discriminant in the first byte
        // See https://doc.rust-lang.org/reference/items/enumerations.html#pointer-casting
        unsafe { *(self as *const Self as *const u8) }
    }

    pub fn serialize<W: Write>(&self, w: &mut WriteAdaptor<W>) -> serializer::Result<()> {
        w.write_u8_be(self.get_type_id())?;
        self.serialize_data(w)?;
        Ok(())
    }

    pub fn serialize_data<W: Write>(&self, w: &mut WriteAdaptor<W>) -> serializer::Result<()> {
        match self {
            NbtTag::End => {}
            NbtTag::Byte(byte) => w.write_i8_be(*byte)?,
            NbtTag::Short(short) => w.write_i16_be(*short)?,
            NbtTag::Int(int) => w.write_i32_be(*int)?,
            NbtTag::Long(long) => w.write_i64_be(*long)?,
            NbtTag::Float(float) => w.write_f32_be(*float)?,
            NbtTag::Double(double) => w.write_f64_be(*double)?,
            NbtTag::ByteArray(byte_array) => {
                let len = byte_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32_be(len as i32)?;
                w.write_slice(byte_array)?;
            }
            NbtTag::String(string) => {
                let java_string = cesu8::to_java_cesu8(string);
                let len = java_string.len();
                if len > u16::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_u16_be(len as u16)?;
                w.write_slice(&java_string)?;
            }
            NbtTag::List(list) => {
                let len = list.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                let element_type = list_element_type(list)?;
                w.write_u8_be(element_type)?;
                w.write_i32_be(len as i32)?;
                for nbt_tag in list {
                    match nbt_tag {
                        NbtTag::Compound(compound)
                            if element_type == COMPOUND_ID && !is_wrapper(compound) =>
                        {
                            compound.serialize_content(w)?
                        }
                        _ if element_type == COMPOUND_ID => {
                            w.write_u8_be(nbt_tag.get_type_id())?;
                            w.write_u16_be(0)?;
                            nbt_tag.serialize_data(w)?;
                            w.write_u8_be(END_ID)?;
                        }
                        _ => nbt_tag.serialize_data(w)?,
                    }
                }
            }
            NbtTag::Compound(compound) => {
                compound.serialize_content(w)?;
            }
            NbtTag::IntArray(int_array) => {
                let len = int_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32_be(len as i32)?;
                for int in int_array {
                    w.write_i32_be(*int)?;
                }
            }
            NbtTag::LongArray(long_array) => {
                let len = long_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32_be(len as i32)?;

                for long in long_array {
                    w.write_i64_be(*long)?;
                }
            }
        };
        Ok(())
    }

    pub fn deserialize<R: Read + Seek>(reader: &mut NbtReadHelper<R>) -> Result<NbtTag, Error> {
        let tag_id = reader.get_u8_be()?;
        Self::deserialize_data(reader, tag_id)
    }

    pub fn skip_data<R: Read + Seek>(
        reader: &mut NbtReadHelper<R>,
        tag_id: u8,
    ) -> Result<(), Error> {
        match tag_id {
            END_ID => Ok(()),
            BYTE_ID => reader.skip_bytes(1),
            SHORT_ID => reader.skip_bytes(2),
            INT_ID => reader.skip_bytes(4),
            LONG_ID => reader.skip_bytes(8),
            FLOAT_ID => reader.skip_bytes(4),
            DOUBLE_ID => reader.skip_bytes(8),
            BYTE_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }
                reader.skip_bytes(len as i64)
            }
            STRING_ID => {
                let len = reader.get_u16_be()?;
                reader.skip_bytes(len as i64)
            }
            LIST_ID => {
                let tag_type_id = reader.get_u8_be()?;
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                reader.push_depth()?;
                for _ in 0..len {
                    Self::skip_data(reader, tag_type_id)?;
                }
                reader.pop_depth();

                Ok(())
            }
            COMPOUND_ID => {
                reader.push_depth()?;
                NbtCompound::skip_content(reader)?;
                reader.pop_depth();
                Ok(())
            }
            INT_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                reader.skip_bytes(len as i64 * 4)
            }
            LONG_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                reader.skip_bytes(len as i64 * 8)
            }
            _ => Err(Error::UnknownTagId(tag_id)),
        }
    }

    pub fn deserialize_data<R: Read + Seek>(
        reader: &mut NbtReadHelper<R>,
        tag_id: u8,
    ) -> Result<NbtTag, Error> {
        match tag_id {
            END_ID => Ok(NbtTag::End),
            BYTE_ID => {
                let byte = reader.get_i8_be()?;
                Ok(NbtTag::Byte(byte))
            }
            SHORT_ID => {
                let short = reader.get_i16_be()?;
                Ok(NbtTag::Short(short))
            }
            INT_ID => {
                let int = reader.get_i32_be()?;
                Ok(NbtTag::Int(int))
            }
            LONG_ID => {
                let long = reader.get_i64_be()?;
                Ok(NbtTag::Long(long))
            }
            FLOAT_ID => {
                let float = reader.get_f32_be()?;
                Ok(NbtTag::Float(float))
            }
            DOUBLE_ID => {
                let double = reader.get_f64_be()?;
                Ok(NbtTag::Double(double))
            }
            BYTE_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let byte_array = reader.read_boxed_slice(len as usize)?;
                Ok(NbtTag::ByteArray(byte_array))
            }
            STRING_ID => Ok(NbtTag::String(get_nbt_string(reader)?)),
            LIST_ID => {
                let tag_type_id = reader.get_u8_be()?;
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                reader.push_depth()?;
                let mut list = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    list.push(match NbtTag::deserialize_data(reader, tag_type_id)? {
                        NbtTag::Compound(mut compound) if is_wrapper(&compound) => {
                            compound.child_tags.pop().unwrap().1
                        }
                        tag => tag,
                    });
                }
                reader.pop_depth();
                Ok(NbtTag::List(list))
            }
            COMPOUND_ID => {
                reader.push_depth()?;
                let compound = NbtCompound::deserialize_content(reader)?;
                reader.pop_depth();
                Ok(NbtTag::Compound(compound))
            }
            INT_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                let mut int_array = Vec::with_capacity(len);
                for _ in 0..len {
                    let int = reader.get_i32_be()?;
                    int_array.push(int);
                }
                Ok(NbtTag::IntArray(int_array))
            }
            LONG_ARRAY_ID => {
                let len = reader.get_i32_be()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                let mut long_array = Vec::with_capacity(len);
                for _ in 0..len {
                    let long = reader.get_i64_be()?;
                    long_array.push(long);
                }
                Ok(NbtTag::LongArray(long_array))
            }
            _ => Err(Error::UnknownTagId(tag_id)),
        }
    }

    pub fn extract_byte(&self) -> Option<i8> {
        match self {
            NbtTag::Byte(byte) => Some(*byte),
            _ => None,
        }
    }

    pub fn extract_short(&self) -> Option<i16> {
        match self {
            NbtTag::Short(short) => Some(*short),
            _ => None,
        }
    }

    pub fn extract_int(&self) -> Option<i32> {
        match self {
            NbtTag::Int(int) => Some(*int),
            _ => None,
        }
    }

    pub fn extract_long(&self) -> Option<i64> {
        match self {
            NbtTag::Long(long) => Some(*long),
            _ => None,
        }
    }

    pub fn extract_float(&self) -> Option<f32> {
        match self {
            NbtTag::Float(float) => Some(*float),
            _ => None,
        }
    }

    pub fn extract_double(&self) -> Option<f64> {
        match self {
            NbtTag::Double(double) => Some(*double),
            _ => None,
        }
    }

    pub fn extract_bool(&self) -> Option<bool> {
        match self {
            NbtTag::Byte(byte) => Some(*byte != 0),
            _ => None,
        }
    }

    pub fn extract_string(&self) -> Option<&str> {
        match self {
            NbtTag::String(string) => Some(string),
            _ => None,
        }
    }

    pub fn extract_list(&self) -> Option<&[NbtTag]> {
        match self {
            NbtTag::List(list) => Some(list),
            _ => None,
        }
    }

    pub fn extract_compound(&self) -> Option<&NbtCompound> {
        match self {
            NbtTag::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    pub fn extract_int_array(&self) -> Option<&[i32]> {
        match self {
            NbtTag::IntArray(int_array) => Some(int_array),
            _ => None,
        }
    }
}

/// A mixed list is written as compounds with every non-compound element
/// wrapped as `{"": value}`; a compound that already looks like a wrapper is
/// wrapped again so the reader's single unwrap gives it back unchanged.
fn is_wrapper(compound: &NbtCompound) -> bool {
    matches!(compound.child_tags.as_slice(), [(key, _)] if key.is_empty())
}

fn list_element_type(list: &[NbtTag]) -> Result<u8, Error> {
    let mut element_type = END_ID;
    for tag in list {
        let id = tag.get_type_id();
        if id == END_ID {
            return Err(Error::SerdeError("a list cannot hold TAG_End".to_string()));
        }
        if element_type == END_ID {
            element_type = id;
        } else if element_type != id {
            return Ok(COMPOUND_ID);
        }
    }
    Ok(element_type)
}

impl From<&str> for NbtTag {
    fn from(value: &str) -> Self {
        NbtTag::String(value.to_string())
    }
}

impl From<f32> for NbtTag {
    fn from(value: f32) -> Self {
        NbtTag::Float(value)
    }
}

impl From<f64> for NbtTag {
    fn from(value: f64) -> Self {
        NbtTag::Double(value)
    }
}

impl From<bool> for NbtTag {
    fn from(value: bool) -> Self {
        NbtTag::Byte(value as i8)
    }
}

/// A human-readable format has no array tags: `JsonOps` writes them as plain
/// lists of signed numbers.
impl Serialize for NbtTag {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let plain_lists = serializer.is_human_readable();
        match self {
            NbtTag::End => serializer.serialize_unit(),
            NbtTag::Byte(v) => serializer.serialize_i8(*v),
            NbtTag::Short(v) => serializer.serialize_i16(*v),
            NbtTag::Int(v) => serializer.serialize_i32(*v),
            NbtTag::Long(v) => serializer.serialize_i64(*v),
            NbtTag::Float(v) => serializer.serialize_f32(*v),
            NbtTag::Double(v) => serializer.serialize_f64(*v),
            NbtTag::ByteArray(v) if plain_lists => {
                serializer.collect_seq(v.iter().map(|b| *b as i8))
            }
            NbtTag::ByteArray(v) => {
                serializer.serialize_newtype_variant(NBT_ARRAY_TAG, 0, NBT_BYTE_ARRAY_TAG, v)
            }
            NbtTag::String(v) => serializer.serialize_str(v),
            NbtTag::List(v) => serializer.collect_seq(v),
            NbtTag::Compound(v) => v.serialize(serializer),
            NbtTag::IntArray(v) if plain_lists => serializer.collect_seq(v),
            NbtTag::IntArray(v) => {
                serializer.serialize_newtype_variant(NBT_ARRAY_TAG, 0, NBT_INT_ARRAY_TAG, v)
            }
            NbtTag::LongArray(v) if plain_lists => serializer.collect_seq(v),
            NbtTag::LongArray(v) => {
                serializer.serialize_newtype_variant(NBT_ARRAY_TAG, 0, NBT_LONG_ARRAY_TAG, v)
            }
        }
    }
}

impl<'de> Deserialize<'de> for NbtTag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NbtTagVisitor {
            json_numbers: bool,
        }

        impl NbtTagVisitor {
            fn integer(&self, v: i64) -> NbtTag {
                if !self.json_numbers {
                    return NbtTag::Long(v);
                }
                if let Ok(v) = i8::try_from(v) {
                    NbtTag::Byte(v)
                } else if let Ok(v) = i16::try_from(v) {
                    NbtTag::Short(v)
                } else if let Ok(v) = i32::try_from(v) {
                    NbtTag::Int(v)
                } else {
                    NbtTag::Long(v)
                }
            }

            fn float(&self, v: f64) -> NbtTag {
                if !self.json_numbers {
                    return NbtTag::Double(v);
                }
                if v.fract() == 0.0 && (-9223372036854775808.0..9223372036854775808.0).contains(&v)
                {
                    self.integer(v as i64)
                } else if (v as f32) as f64 == v {
                    NbtTag::Float(v as f32)
                } else {
                    NbtTag::Double(v)
                }
            }
        }

        impl<'de> serde::de::Visitor<'de> for NbtTagVisitor {
            type Value = NbtTag;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an NBT tag")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(NbtTag::Byte(v as i8))
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E> {
                Ok(NbtTag::Byte(v))
            }

            fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E> {
                Ok(NbtTag::Short(v))
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E> {
                Ok(NbtTag::Int(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(self.integer(v))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                match i64::try_from(v) {
                    Ok(v) => Ok(self.integer(v)),
                    Err(_) if self.json_numbers => Ok(self.float(v as f64)),
                    Err(err) => Err(E::custom(err)),
                }
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E> {
                Ok(NbtTag::Float(v))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                Ok(self.float(v))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(NbtTag::String(v.to_string()))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut vec = Vec::new();
                while let Some(value) = seq.next_element()? {
                    vec.push(value);
                }
                Ok(NbtTag::List(vec))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(NbtTag::Compound(NbtCompound::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )?))
            }

            fn visit_newtype_struct<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                deserializer.deserialize_any(self)
            }

            fn visit_enum<A: serde::de::EnumAccess<'de>>(
                self,
                data: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::VariantAccess;
                let (variant, access): (String, _) = data.variant()?;
                match variant.as_str() {
                    NBT_BYTE_ARRAY_TAG => Ok(NbtTag::ByteArray(
                        access.newtype_variant::<Vec<u8>>()?.into_boxed_slice(),
                    )),
                    NBT_INT_ARRAY_TAG => Ok(NbtTag::IntArray(access.newtype_variant()?)),
                    NBT_LONG_ARRAY_TAG => Ok(NbtTag::LongArray(access.newtype_variant()?)),
                    other => Err(serde::de::Error::unknown_variant(
                        other,
                        &[NBT_BYTE_ARRAY_TAG, NBT_INT_ARRAY_TAG, NBT_LONG_ARRAY_TAG],
                    )),
                }
            }
        }

        let visitor = NbtTagVisitor {
            json_numbers: deserializer.is_human_readable() && !crate::reading_binary(),
        };
        deserializer.deserialize_newtype_struct(NBT_ARRAY_TAG, visitor)
    }
}
