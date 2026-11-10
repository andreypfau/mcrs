use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;

use crate::encoding::{BigEndian, NbtEncoding};
use crate::tag::{self, NbtTag};
use crate::{
    BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, DOUBLE_ID, END_ID, Error, FLOAT_ID, INT_ARRAY_ID, INT_ID,
    LIST_ID, LONG_ARRAY_ID, LONG_ID, NBT_ARRAY_TAG, NBT_BYTE_ARRAY_TAG, NBT_INT_ARRAY_TAG,
    NBT_LONG_ARRAY_TAG, SHORT_ID, STRING_ID, get_nbt_string,
};
use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, forward_to_deserialize_any};

pub type Result<T> = std::result::Result<T, Error>;

/// A declared length is a claim the input has yet to back: a buffer sized by
/// one grows this far at a time, so the room set aside stays within about
/// twice the bytes that have actually arrived.
pub(crate) const READ_STEP: usize = 64 * 1024;

/// The room to set aside for a declared number of elements before any of them
/// has been read.
pub fn cautious_capacity<T>(declared: usize, max_bytes: usize) -> usize {
    declared.min(max_bytes / size_of::<T>().max(1))
}

#[derive(Debug)]
pub struct NbtReadHelper<R: Read + Seek, E: NbtEncoding = BigEndian> {
    pub(crate) reader: R,
    depth: u32,
    _encoding: PhantomData<E>,
}

impl<R: Read + Seek> NbtReadHelper<R, BigEndian> {
    pub fn new(r: R) -> Self {
        Self::with_encoding(r)
    }
}

impl<R: Read + Seek, E: NbtEncoding> NbtReadHelper<R, E> {
    pub fn with_encoding(r: R) -> Self {
        Self {
            reader: r,
            depth: 0,
            _encoding: PhantomData,
        }
    }

    pub fn push_depth(&mut self) -> Result<()> {
        if self.depth >= crate::MAX_DEPTH {
            return Err(Error::TooDeep);
        }
        self.depth += 1;
        Ok(())
    }

    pub fn pop_depth(&mut self) {
        self.depth -= 1;
    }

    pub fn skip_bytes(&mut self, count: i64) -> Result<()> {
        if count > 1 {
            self.reader
                .by_ref()
                .seek(SeekFrom::Current(count - 1))
                .map_err(Error::Incomplete)?;
        }
        if count > 0 {
            self.get_u8()?;
        }
        Ok(())
    }

    #[inline]
    pub fn get_u8(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.reader
            .read_exact(&mut buf)
            .map_err(Error::Incomplete)?;
        Ok(buf[0])
    }

    #[inline]
    pub fn get_i8(&mut self) -> Result<i8> {
        let mut buf = [0u8; 1];
        self.reader
            .read_exact(&mut buf)
            .map_err(Error::Incomplete)?;
        Ok(buf[0] as i8)
    }

    #[inline]
    pub fn get_u16(&mut self) -> Result<u16> {
        E::read_u16(&mut self.reader)
    }

    #[inline]
    pub fn get_i16(&mut self) -> Result<i16> {
        E::read_i16(&mut self.reader)
    }

    #[inline]
    pub fn get_i32(&mut self) -> Result<i32> {
        E::read_i32(&mut self.reader)
    }

    #[inline]
    pub fn get_i64(&mut self) -> Result<i64> {
        E::read_i64(&mut self.reader)
    }

    #[inline]
    pub fn get_f32(&mut self) -> Result<f32> {
        E::read_f32(&mut self.reader)
    }

    #[inline]
    pub fn get_f64(&mut self) -> Result<f64> {
        E::read_f64(&mut self.reader)
    }

    pub fn read_into(&mut self, buf: &mut Vec<u8>, count: usize) -> Result<()> {
        buf.clear();
        while buf.len() < count {
            let filled = buf.len();
            buf.resize(count.min(filled + READ_STEP), 0);
            self.reader
                .read_exact(buf.get_mut(filled..).unwrap_or_default())
                .map_err(Error::Incomplete)?;
        }
        Ok(())
    }

    pub fn position(&mut self) -> Result<u64> {
        self.reader.stream_position().map_err(Error::Incomplete)
    }

    pub fn seek_to(&mut self, position: u64) -> Result<()> {
        self.reader
            .seek(SeekFrom::Start(position))
            .map_err(Error::Incomplete)?;
        Ok(())
    }

    pub fn read_boxed_slice(&mut self, count: usize) -> Result<Box<[u8]>> {
        let mut buf = Vec::with_capacity(cautious_capacity::<u8>(count, READ_STEP));
        self.read_into(&mut buf, count)?;
        Ok(buf.into())
    }
}

#[derive(Debug)]
pub struct Deserializer<R: Read + Seek, E: NbtEncoding = BigEndian> {
    input: NbtReadHelper<R, E>,
    tag_to_deserialize_stack: Option<u8>,
    in_list: bool,
    is_named: bool,
    scratch: Vec<u8>,
    _binary: crate::BinaryReadGuard,
}

impl<R: Read + Seek> Deserializer<R, BigEndian> {
    pub fn new(input: R, is_named: bool) -> Self {
        Self::with_encoding(input, is_named)
    }
}

impl<R: Read + Seek, E: NbtEncoding> Deserializer<R, E> {
    pub fn with_encoding(input: R, is_named: bool) -> Self {
        Deserializer {
            input: NbtReadHelper::with_encoding(input),
            tag_to_deserialize_stack: None,
            in_list: false,
            is_named,
            scratch: Vec::new(),
            _binary: crate::BinaryReadGuard::new(),
        }
    }

    fn read_str(&mut self) -> Result<Cow<'_, str>> {
        let len = E::read_string_length(&mut self.input.reader)?;
        self.input.read_into(&mut self.scratch, len)?;
        E::decode_string(&self.scratch)
    }

    fn read_root(&mut self) -> Result<u8> {
        if let Some(tag) = self.tag_to_deserialize_stack {
            return Ok(tag);
        }
        let tag = self.input.get_u8()?;
        if tag == END_ID {
            return Err(Error::EndRoot);
        }
        if self.is_named {
            if tag != COMPOUND_ID {
                return Err(Error::NoRootCompound(tag));
            }
            let length = E::read_string_length(&mut self.input.reader)? as i64;
            self.input.skip_bytes(length)?;
        }
        self.tag_to_deserialize_stack = Some(tag);
        Ok(tag)
    }

    fn visit_compound<'de, V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value> {
        self.input.push_depth()?;
        let result = visitor.visit_map(CompoundAccess { de: self });
        self.input.pop_depth();
        result
    }
}

/// Deserializes struct using Serde Deserializer from normal NBT with standard BigEndian encoding
pub fn from_bytes<'a, T: Deserialize<'a>>(r: impl Read + Seek) -> Result<T> {
    from_bytes_with::<BigEndian, T>(r)
}

/// Deserializes struct using Serde Deserializer from normal NBT with specified encoding
pub fn from_bytes_with<'a, E: NbtEncoding, T: Deserialize<'a>>(r: impl Read + Seek) -> Result<T> {
    let mut deserializer = Deserializer::<_, E>::with_encoding(r, true);
    T::deserialize(&mut deserializer)
}

/// Deserializes struct using Serde Deserializer from network NBT with standard BigEndian encoding
pub fn from_bytes_unnamed<'a, T: Deserialize<'a>>(r: impl Read + Seek) -> Result<T> {
    from_bytes_unnamed_with::<BigEndian, T>(r)
}

/// Deserializes struct using Serde Deserializer from network NBT with specified encoding
pub fn from_bytes_unnamed_with<'a, E: NbtEncoding, T: Deserialize<'a>>(
    r: impl Read + Seek,
) -> Result<T> {
    let mut deserializer = Deserializer::<_, E>::with_encoding(r, false);
    T::deserialize(&mut deserializer)
}

macro_rules! define_in_list_number {
    ($name:ident, $tag:expr, $read:ident, $visit:ident) => {
        fn $name<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            if self.in_list && self.tag_to_deserialize_stack == Some($tag) {
                let value = self.input.$read()?;
                return visitor.$visit::<Error>(value);
            }
            let tag = self.read_root()?;
            match tag {
                BYTE_ID | SHORT_ID | INT_ID | LONG_ID | FLOAT_ID | DOUBLE_ID => {
                    NbtTag::deserialize_data(&mut self.input, tag)?.$name(visitor)
                }
                _ => self.deserialize_any(visitor),
            }
        }
    };
}

impl<'de, R: Read + Seek, E: NbtEncoding> de::Deserializer<'de> for &mut Deserializer<R, E> {
    type Error = Error;

    forward_to_deserialize_any! {
        char str string unit unit_struct seq tuple tuple_struct identifier
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        if name != NBT_ARRAY_TAG {
            return visitor.visit_newtype_struct(self);
        }
        self.read_root()?;
        let variant = match self.tag_to_deserialize_stack {
            Some(BYTE_ARRAY_ID) => NBT_BYTE_ARRAY_TAG,
            Some(INT_ARRAY_ID) => NBT_INT_ARRAY_TAG,
            Some(LONG_ARRAY_ID) => NBT_LONG_ARRAY_TAG,
            _ => return visitor.visit_newtype_struct(self),
        };
        visitor.visit_enum(ArrayAccess { de: self, variant })
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
        let width = match self.tag_to_deserialize_stack {
            Some(BYTE_ARRAY_ID) => 1,
            Some(INT_ARRAY_ID) if E::fixed_size(INT_ID).is_some() => 4,
            Some(LONG_ARRAY_ID) if E::fixed_size(LONG_ID).is_some() => 8,
            _ => return self.deserialize_any(visitor),
        };
        let count = E::read_collection_length(&mut self.input.reader)?;
        let bytes = count.checked_mul(width).ok_or(Error::LargeLength(count))?;
        self.input.read_into(&mut self.scratch, bytes)?;
        visitor.visit_bytes(&self.scratch)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    define_in_list_number!(deserialize_i8, BYTE_ID, get_i8, visit_i8);
    define_in_list_number!(deserialize_i16, SHORT_ID, get_i16, visit_i16);
    define_in_list_number!(deserialize_i32, INT_ID, get_i32, visit_i32);
    define_in_list_number!(deserialize_i64, LONG_ID, get_i64, visit_i64);
    define_in_list_number!(deserialize_f32, FLOAT_ID, get_f32, visit_f32);
    define_in_list_number!(deserialize_f64, DOUBLE_ID, get_f64, visit_f64);

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag = self.read_root()?;
        NbtTag::skip_data(&mut self.input, tag)?;
        visitor.visit_unit()
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag_to_deserialize = self.read_root()?;

        match tag_to_deserialize {
            END_ID => Err(Error::SerdeError(
                "Trying to deserialize an END tag!".to_string(),
            )),
            LIST_ID | INT_ARRAY_ID | LONG_ARRAY_ID | BYTE_ARRAY_ID => {
                let (list_type, remaining_values) = if tag_to_deserialize == LIST_ID {
                    tag::read_list_header(&mut self.input)?
                } else {
                    let list_type = match tag_to_deserialize {
                        INT_ARRAY_ID => INT_ID,
                        LONG_ARRAY_ID => LONG_ID,
                        _ => BYTE_ID,
                    };
                    let count = E::read_collection_length(&mut self.input.reader)?;
                    (list_type, count)
                };

                self.input.push_depth()?;
                let result = visitor.visit_seq(ListAccess {
                    de: self,
                    list_type,
                    remaining_values,
                });
                self.input.pop_depth();
                result
            }
            COMPOUND_ID => self.visit_compound(visitor),
            STRING_ID => {
                let value = self.read_str()?;
                visitor.visit_str(&value)
            }
            _ => {
                let result = match NbtTag::deserialize_data(&mut self.input, tag_to_deserialize)? {
                    NbtTag::Byte(value) => visitor.visit_i8::<Error>(value)?,
                    NbtTag::Short(value) => visitor.visit_i16::<Error>(value)?,
                    NbtTag::Int(value) => visitor.visit_i32::<Error>(value)?,
                    NbtTag::Long(value) => visitor.visit_i64::<Error>(value)?,
                    NbtTag::Float(value) => visitor.visit_f32::<Error>(value)?,
                    NbtTag::Double(value) => visitor.visit_f64::<Error>(value)?,
                    other => {
                        return Err(Error::SerdeError(format!(
                            "tag {} is not a number",
                            other.get_type_id()
                        )));
                    }
                };
                Ok(result)
            }
        }
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.in_list && self.tag_to_deserialize_stack == Some(BYTE_ID) {
            let value = self.input.get_u8()?;
            return visitor.visit_u8::<Error>(value);
        }
        Err(Error::UnsupportedType(
            "u8; NBT only supports signed values".to_string(),
        ))
    }

    fn deserialize_u16<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::UnsupportedType(
            "u16; NBT only supports signed values".to_string(),
        ))
    }

    fn deserialize_u32<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::UnsupportedType(
            "u32; NBT only supports signed values".to_string(),
        ))
    }

    fn deserialize_u64<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::UnsupportedType(
            "u64; NBT only supports signed values".to_string(),
        ))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag = self.read_root()?;
        NbtTag::deserialize_data(&mut self.input, tag)?.deserialize_bool(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.read_root()?;
        if self.tag_to_deserialize_stack != Some(STRING_ID) {
            return self.deserialize_any(visitor);
        }
        let variant = get_nbt_string(&mut self.input)?;
        visitor.visit_enum(variant.into_deserializer())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag_id = self.read_root()?;
        if tag_id != COMPOUND_ID {
            return Err(Error::SerdeError(format!(
                "Trying to deserialize a map without a compound ID (id {tag_id})"
            )));
        }

        self.visit_compound(visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_map(visitor)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct CompoundAccess<'a, R: Read + Seek, E: NbtEncoding> {
    de: &'a mut Deserializer<R, E>,
}

impl<'de, R: Read + Seek, E: NbtEncoding> MapAccess<'de> for CompoundAccess<'_, R, E> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let tag = self.de.input.get_u8()?;
        self.de.tag_to_deserialize_stack = Some(tag);

        if tag == END_ID {
            return Ok(None);
        }

        seed.deserialize(MapKey { de: self.de }).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(&mut *self.de)
    }
}

struct ArrayAccess<'a, R: Read + Seek, E: NbtEncoding> {
    de: &'a mut Deserializer<R, E>,
    variant: &'static str,
}

impl<'de, 'a, R: Read + Seek, E: NbtEncoding> de::EnumAccess<'de> for ArrayAccess<'a, R, E> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de, R: Read + Seek, E: NbtEncoding> de::VariantAccess<'de> for ArrayAccess<'_, R, E> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::UnsupportedType("array as unit variant".to_string()))
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(&mut *self.de)
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

struct MapKey<'a, R: Read + Seek, E: NbtEncoding> {
    de: &'a mut Deserializer<R, E>,
}

impl<'de, R: Read + Seek, E: NbtEncoding> de::Deserializer<'de> for MapKey<'_, R, E> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let key = self.de.read_str()?;
        visitor.visit_str(&key)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit unit_struct seq tuple tuple_struct map
        struct identifier ignored_any bytes enum newtype_struct byte_buf option
    }
}

const MAX_SIZE_HINT: usize = 4096;

struct ListAccess<'a, R: Read + Seek, E: NbtEncoding> {
    de: &'a mut Deserializer<R, E>,
    remaining_values: usize,
    list_type: u8,
}

impl<'de, R: Read + Seek, E: NbtEncoding> SeqAccess<'de> for ListAccess<'_, R, E> {
    type Error = Error;

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining_values.min(MAX_SIZE_HINT))
    }

    fn next_element_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<Option<V::Value>> {
        if self.remaining_values == 0 {
            return Ok(None);
        }

        self.remaining_values -= 1;
        let wrapped = match self.list_type {
            COMPOUND_ID => wrapper_payload(&mut self.de.input)?,
            _ => None,
        };
        self.de.tag_to_deserialize_stack = Some(wrapped.unwrap_or(self.list_type));
        self.de.in_list = true;
        let result = seed.deserialize(&mut *self.de).map(Some);
        self.de.in_list = false;
        if result.is_ok() && wrapped.is_some() {
            self.de.input.skip_bytes(1)?;
        }

        result
    }
}

fn wrapper_payload<R: Read + Seek, E: NbtEncoding>(
    input: &mut NbtReadHelper<R, E>,
) -> Result<Option<u8>> {
    let start = input.position()?;
    let tag = input.get_u8()?;
    if tag != END_ID && E::read_string_length(&mut input.reader)? == 0 {
        let payload = input.position()?;
        NbtTag::skip_data(input, tag)?;
        if input.get_u8()? == END_ID {
            input.seek_to(payload)?;
            return Ok(Some(tag));
        }
    }
    input.seek_to(start)?;
    Ok(None)
}
