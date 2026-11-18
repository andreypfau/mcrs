use crate::compound::NbtCompound;
use crate::deserializer::{NbtReadHelper, READ_STEP, cautious_capacity};
use crate::encoding::NbtEncoding;
use crate::serializer::{self, WriteAdaptor};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, Write};
use std::mem;

use crate::{
    BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, DOUBLE_ID, END_ID, Error, FLOAT_ID, INT_ARRAY_ID, INT_ID,
    LIST_ID, LONG_ARRAY_ID, LONG_ID, NBT_ARRAY_TAG, NBT_BYTE_ARRAY_TAG, NBT_INT_ARRAY_TAG,
    NBT_LONG_ARRAY_TAG, SHORT_ID, STRING_ID, get_nbt_string,
};

#[derive(Clone, Debug)]
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

fn float_identity(value: f32) -> u32 {
    if value.is_nan() {
        f32::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

fn double_identity(value: f64) -> u64 {
    if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

impl PartialEq for NbtTag {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NbtTag::End, NbtTag::End) => true,
            (NbtTag::Byte(left), NbtTag::Byte(right)) => left == right,
            (NbtTag::Short(left), NbtTag::Short(right)) => left == right,
            (NbtTag::Int(left), NbtTag::Int(right)) => left == right,
            (NbtTag::Long(left), NbtTag::Long(right)) => left == right,
            (NbtTag::Float(left), NbtTag::Float(right)) => {
                float_identity(*left) == float_identity(*right)
            }
            (NbtTag::Double(left), NbtTag::Double(right)) => {
                double_identity(*left) == double_identity(*right)
            }
            (NbtTag::ByteArray(left), NbtTag::ByteArray(right)) => left == right,
            (NbtTag::String(left), NbtTag::String(right)) => left == right,
            (NbtTag::List(left), NbtTag::List(right)) => left == right,
            (NbtTag::Compound(left), NbtTag::Compound(right)) => left == right,
            (NbtTag::IntArray(left), NbtTag::IntArray(right)) => left == right,
            (NbtTag::LongArray(left), NbtTag::LongArray(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for NbtTag {}

impl Hash for NbtTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        mem::discriminant(self).hash(state);
        match self {
            NbtTag::End => {}
            NbtTag::Byte(value) => value.hash(state),
            NbtTag::Short(value) => value.hash(state),
            NbtTag::Int(value) => value.hash(state),
            NbtTag::Long(value) => value.hash(state),
            NbtTag::Float(value) => float_identity(*value).hash(state),
            NbtTag::Double(value) => double_identity(*value).hash(state),
            NbtTag::ByteArray(value) => value.hash(state),
            NbtTag::String(value) => value.hash(state),
            NbtTag::List(value) => value.hash(state),
            NbtTag::Compound(value) => value.hash(state),
            NbtTag::IntArray(value) => value.hash(state),
            NbtTag::LongArray(value) => value.hash(state),
        }
    }
}

impl NbtTag {
    #[must_use]
    pub fn depth(&self, budget: u32) -> u32 {
        match self {
            NbtTag::Compound(compound) => compound.depth(budget),
            NbtTag::List(list) => list_depth(list, budget),
            _ => 0,
        }
    }

    pub fn matches(&self, actual: &NbtTag) -> bool {
        match (self, actual) {
            (NbtTag::Compound(expected), NbtTag::Compound(actual)) => expected.matches(actual),
            (NbtTag::List(expected), NbtTag::List(actual)) if expected.is_empty() => {
                actual.is_empty()
            }
            (NbtTag::List(expected), NbtTag::List(actual)) => {
                actual.len() >= expected.len()
                    && expected
                        .iter()
                        .all(|element| actual.iter().any(|candidate| element.matches(candidate)))
            }
            (expected, actual) => expected == actual,
        }
    }

    pub const fn get_type_id(&self) -> u8 {
        // SAFETY: `NbtTag` is `repr(u8)`, so its first byte is the discriminant.
        unsafe { *(self as *const Self as *const u8) }
    }

    pub fn serialize<W: Write, E: NbtEncoding>(
        &self,
        w: &mut WriteAdaptor<W, E>,
    ) -> serializer::Result<()> {
        w.write_u8(self.get_type_id())?;
        self.serialize_data(w)?;
        Ok(())
    }

    pub fn serialize_data<W: Write, E: NbtEncoding>(
        &self,
        w: &mut WriteAdaptor<W, E>,
    ) -> serializer::Result<()> {
        match self {
            NbtTag::List(list) => serialize_list(list, w),
            NbtTag::Compound(compound) => compound.serialize_content(w),
            _ => self.serialize_flat_data(w),
        }
    }

    #[inline(never)]
    fn serialize_flat_data<W: Write, E: NbtEncoding>(
        &self,
        w: &mut WriteAdaptor<W, E>,
    ) -> serializer::Result<()> {
        match self {
            NbtTag::End => {}
            NbtTag::Byte(byte) => w.write_i8(*byte)?,
            NbtTag::Short(short) => w.write_i16(*short)?,
            NbtTag::Int(int) => w.write_i32(*int)?,
            NbtTag::Long(long) => w.write_i64(*long)?,
            NbtTag::Float(float) => w.write_f32(*float)?,
            NbtTag::Double(double) => w.write_f64(*double)?,
            NbtTag::ByteArray(byte_array) => {
                let len = byte_array.len();
                E::write_collection_length(w, len)?;
                w.write_slice(byte_array)?;
            }
            NbtTag::String(string) => {
                E::write_string(w, string)?;
            }
            NbtTag::List(_) | NbtTag::Compound(_) => self.serialize_data(w)?,
            NbtTag::IntArray(int_array) => {
                let len = int_array.len();
                E::write_collection_length(w, len)?;
                for int in int_array {
                    w.write_i32(*int)?;
                }
            }
            NbtTag::LongArray(long_array) => {
                let len = long_array.len();
                E::write_collection_length(w, len)?;
                for long in long_array {
                    w.write_i64(*long)?;
                }
            }
        };
        Ok(())
    }

    pub fn deserialize<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
    ) -> Result<NbtTag, Error> {
        let tag_id = reader.read_root_kind()?;
        Self::deserialize_data(reader, tag_id)
    }

    pub fn skip_data<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
        tag_id: u8,
    ) -> Result<(), Error> {
        if let Some(size) = E::fixed_size(tag_id) {
            return reader.skip_bytes(size);
        }
        match tag_id {
            END_ID => Ok(()),
            BYTE_ARRAY_ID => {
                let len = reader.read_array_length(1)?;
                reader.skip_bytes(len as i64)
            }
            STRING_ID => {
                let len = reader.read_string_length()?;
                reader.skip_bytes(len as i64)
            }
            INT_ID => {
                E::read_i32(&mut reader.reader)?;
                Ok(())
            }
            LONG_ID => {
                E::read_i64(&mut reader.reader)?;
                Ok(())
            }
            LIST_ID => {
                let (tag_type_id, len) = read_list_header(reader)?;
                if let Some(size) = E::fixed_size(tag_type_id) {
                    return reader.skip_bytes(len as i64 * size);
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
                let len = reader.read_array_length(4)?;
                if let Some(size) = E::fixed_size(INT_ID) {
                    reader.skip_bytes(len as i64 * size)
                } else {
                    for _ in 0..len {
                        E::read_i32(&mut reader.reader)?;
                    }
                    Ok(())
                }
            }
            LONG_ARRAY_ID => {
                let len = reader.read_array_length(8)?;
                if let Some(size) = E::fixed_size(LONG_ID) {
                    reader.skip_bytes(len as i64 * size)
                } else {
                    for _ in 0..len {
                        E::read_i64(&mut reader.reader)?;
                    }
                    Ok(())
                }
            }
            _ => Err(Error::UnknownTagId(tag_id)),
        }
    }

    pub fn deserialize_data<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
        tag_id: u8,
    ) -> Result<NbtTag, Error> {
        match tag_id {
            LIST_ID => deserialize_list(reader).map(NbtTag::List),
            COMPOUND_ID => deserialize_compound(reader).map(NbtTag::Compound),
            _ => Self::deserialize_flat_data(reader, tag_id),
        }
    }

    #[inline(never)]
    fn deserialize_flat_data<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
        tag_id: u8,
    ) -> Result<NbtTag, Error> {
        match tag_id {
            END_ID => Ok(NbtTag::End),
            BYTE_ID => {
                let byte = reader.get_i8()?;
                Ok(NbtTag::Byte(byte))
            }
            SHORT_ID => {
                let short = reader.get_i16()?;
                Ok(NbtTag::Short(short))
            }
            INT_ID => {
                let int = reader.get_i32()?;
                Ok(NbtTag::Int(int))
            }
            LONG_ID => {
                let long = reader.get_i64()?;
                Ok(NbtTag::Long(long))
            }
            FLOAT_ID => {
                let float = reader.get_f32()?;
                Ok(NbtTag::Float(float))
            }
            DOUBLE_ID => {
                let double = reader.get_f64()?;
                Ok(NbtTag::Double(double))
            }
            BYTE_ARRAY_ID => {
                let len = reader.read_array_length(1)?;
                let byte_array = reader.read_boxed_slice(len)?;
                Ok(NbtTag::ByteArray(byte_array))
            }
            STRING_ID => Ok(NbtTag::String(get_nbt_string(reader)?)),
            LIST_ID | COMPOUND_ID => Self::deserialize_data(reader, tag_id),
            INT_ARRAY_ID => {
                let len = reader.read_array_length(4)?;
                let mut int_array = Vec::with_capacity(cautious_capacity::<i32>(len, READ_STEP));
                for _ in 0..len {
                    let int = reader.get_i32()?;
                    int_array.push(int);
                }
                Ok(NbtTag::IntArray(int_array))
            }
            LONG_ARRAY_ID => {
                let len = reader.read_array_length(8)?;
                let mut long_array = Vec::with_capacity(cautious_capacity::<i64>(len, READ_STEP));
                for _ in 0..len {
                    let long = reader.get_i64()?;
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

pub(crate) fn read_list_header<R: Read + Seek, E: NbtEncoding>(
    reader: &mut NbtReadHelper<R, E>,
) -> Result<(u8, usize), Error> {
    let element_type = reader.get_u8()?;
    let len = E::read_collection_length(&mut reader.reader)?;
    let count = len as u64;
    let element = 4 + crate::budget::value_charge(element_type);
    crate::budget::charge(element.saturating_mul(count).saturating_add(5), count)?;
    if element_type == END_ID && len > 0 {
        return Err(Error::SerdeError("a list cannot hold TAG_End".to_string()));
    }
    Ok((element_type, len))
}

fn is_wrapper(compound: &NbtCompound) -> bool {
    matches!(compound.child_tags.as_slice(), [(key, _)] if key.is_empty())
}

fn is_wrapped_list_element(element_type: u8, element: &NbtTag) -> bool {
    element_type == COMPOUND_ID
        && !matches!(element, NbtTag::Compound(compound) if !is_wrapper(compound))
}

fn list_element_id(list: &[NbtTag]) -> Option<u8> {
    let mut element_type = END_ID;
    for tag in list {
        let id = tag.get_type_id();
        if id == END_ID {
            return None;
        }
        if element_type == END_ID {
            element_type = id;
        } else if element_type != id {
            return Some(COMPOUND_ID);
        }
    }
    Some(element_type)
}

fn list_depth(list: &[NbtTag], budget: u32) -> u32 {
    let Some(budget) = budget.checked_sub(1) else {
        return 1;
    };
    let element_type = list_element_id(list).unwrap_or(END_ID);
    deepest(list.iter().map(|element| {
        let levels = element.depth(budget);
        if is_wrapped_list_element(element_type, element) {
            levels.saturating_add(1)
        } else {
            levels
        }
    }))
}

#[must_use]
pub fn deepest(levels: impl IntoIterator<Item = u32>) -> u32 {
    levels.into_iter().max().unwrap_or(0).saturating_add(1)
}

fn deserialize_list<R: Read + Seek, E: NbtEncoding>(
    reader: &mut NbtReadHelper<R, E>,
) -> Result<Vec<NbtTag>, Error> {
    let (tag_type_id, len) = read_list_header(reader)?;
    reader.push_depth()?;
    let mut list = Vec::with_capacity(cautious_capacity::<NbtTag>(len, READ_STEP));
    for _ in 0..len {
        NbtTag::deserialize_data(reader, tag_type_id).map(|tag| list.push(unwrap_element(tag)))?;
    }
    reader.pop_depth();
    Ok(list)
}

#[inline(never)]
fn unwrap_element(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(mut compound) if is_wrapper(&compound) => {
            match compound.child_tags.pop() {
                Some((_, tag)) => tag,
                None => NbtTag::Compound(compound),
            }
        }
        tag => tag,
    }
}

fn deserialize_compound<R: Read + Seek, E: NbtEncoding>(
    reader: &mut NbtReadHelper<R, E>,
) -> Result<NbtCompound, Error> {
    reader.push_depth()?;
    let compound = NbtCompound::deserialize_content(reader)?;
    reader.pop_depth();
    Ok(compound)
}

fn list_element_type(list: &[NbtTag]) -> Result<u8, Error> {
    list_element_id(list).ok_or_else(|| Error::SerdeError("a list cannot hold TAG_End".to_string()))
}

fn serialize_list<W: Write, E: NbtEncoding>(
    list: &[NbtTag],
    w: &mut WriteAdaptor<W, E>,
) -> serializer::Result<()> {
    let element_type = list_element_type(list)?;
    w.write_u8(element_type)?;
    E::write_collection_length(w, list.len())?;
    for nbt_tag in list {
        if is_wrapped_list_element(element_type, nbt_tag) {
            write_wrapper_header(nbt_tag, w)?;
            nbt_tag.serialize_data(w)?;
            w.write_u8(END_ID)?;
        } else {
            nbt_tag.serialize_data(w)?;
        }
    }
    Ok(())
}

#[inline(never)]
fn write_wrapper_header<W: Write, E: NbtEncoding>(
    nbt_tag: &NbtTag,
    w: &mut WriteAdaptor<W, E>,
) -> serializer::Result<()> {
    w.write_u8(nbt_tag.get_type_id())?;
    E::write_string(w, "")
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
    #[expect(
        clippy::too_many_lines,
        reason = "the visitor type and its impls are local to the deserializer they serve"
    )]
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
