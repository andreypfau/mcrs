use std::borrow::Cow;
use std::io::{Seek, SeekFrom};

use crate::*;
use io::Read;
use serde::de::{self, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, forward_to_deserialize_any};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct NbtReadHelper<R: Read + Seek> {
    reader: R,
}

impl<R: Read + Seek> NbtReadHelper<R> {
    pub fn new(r: R) -> Self {
        Self { reader: r }
    }
}

macro_rules! define_get_number_be {
    ($name:ident, $type:ty) => {
        pub fn $name(&mut self) -> Result<$type> {
            let mut buf = [0u8; std::mem::size_of::<$type>()];
            self.reader
                .read_exact(&mut buf)
                .map_err(Error::Incomplete)?;

            Ok(<$type>::from_be_bytes(buf))
        }
    };
}

impl<R: Read + Seek> NbtReadHelper<R> {
    pub fn skip_bytes(&mut self, count: i64) -> Result<()> {
        self.reader
            .by_ref()
            .seek(SeekFrom::Current(count))
            .map_err(Error::Incomplete)?;
        Ok(())
    }

    define_get_number_be!(get_u8_be, u8);
    define_get_number_be!(get_i8_be, i8);
    define_get_number_be!(get_u16_be, u16);
    define_get_number_be!(get_i16_be, i16);
    define_get_number_be!(get_u32_be, u32);
    define_get_number_be!(get_i32_be, i32);
    define_get_number_be!(get_u64_be, u64);
    define_get_number_be!(get_i64_be, i64);
    define_get_number_be!(get_f32_be, f32);
    define_get_number_be!(get_f64_be, f64);

    /// Fills `buf` with `count` bytes, reusing its allocation.
    pub fn read_into(&mut self, buf: &mut Vec<u8>, count: usize) -> Result<()> {
        buf.clear();
        buf.resize(count, 0);
        self.reader.read_exact(buf).map_err(Error::Incomplete)
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
        let mut buf = vec![0u8; count];
        self.reader
            .read_exact(&mut buf)
            .map_err(Error::Incomplete)?;

        Ok(buf.into())
    }
}

#[derive(Debug)]
pub struct Deserializer<R: Read + Seek> {
    input: NbtReadHelper<R>,
    tag_to_deserialize_stack: Option<u8>,
    // Yes, this breaks with recursion. Just an attempt at a sanity check
    in_list: bool,
    is_named: bool,
    scratch: Vec<u8>,
}

impl<R: Read + Seek> Deserializer<R> {
    pub fn new(input: R, is_named: bool) -> Self {
        Deserializer {
            input: NbtReadHelper { reader: input },
            tag_to_deserialize_stack: None,
            in_list: false,
            is_named,
            scratch: Vec::new(),
        }
    }

    /// Reads one NBT string through the reusable scratch buffer. CESU-8 borrows
    /// for the plain-ASCII case, which is every key and nearly every value, so a
    /// caller that only needs `&str` pays no allocation at all.
    fn read_str(&mut self) -> Result<Cow<'_, str>> {
        let len = self.input.get_u16_be()? as usize;
        self.input.read_into(&mut self.scratch, len)?;
        cesu8::from_java_cesu8(&self.scratch).map_err(|_| Error::Cesu8DecodingError)
    }
}

/// Deserializes struct using Serde Deserializer from normal NBT
pub fn from_bytes<'a, T: Deserialize<'a>>(r: impl Read + Seek) -> Result<T> {
    let mut deserializer = Deserializer::new(r, true);
    T::deserialize(&mut deserializer)
}

/// Deserializes struct using Serde Deserializer from network NBT
pub fn from_bytes_unnamed<'a, T: Deserialize<'a>>(r: impl Read + Seek) -> Result<T> {
    let mut deserializer = Deserializer::new(r, false);
    T::deserialize(&mut deserializer)
}

macro_rules! define_in_list_number {
    ($name:ident, $tag:expr, $read:ident, $visit:ident) => {
        fn $name<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            if self.in_list && self.tag_to_deserialize_stack == Some($tag) {
                let value = self.input.$read()?;
                return visitor.$visit::<Error>(value);
            }
            self.deserialize_any(visitor)
        }
    };
}

impl<'de, R: Read + Seek> de::Deserializer<'de> for &mut Deserializer<R> {
    type Error = Error;

    forward_to_deserialize_any! {
        char str string unit unit_struct seq tuple tuple_struct
        newtype_struct
    }

    // The whole payload goes to the visitor in one piece; read element-wise it
    // costs a visitor round trip each. A plain list still falls through to `any`.
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let width = match self.tag_to_deserialize_stack {
            Some(BYTE_ARRAY_ID) => 1,
            Some(INT_ARRAY_ID) => 4,
            Some(LONG_ARRAY_ID) => 8,
            _ => return self.deserialize_any(visitor),
        };
        let count = self.input.get_i32_be()?;
        if count < 0 {
            return Err(Error::NegativeLength(count));
        }
        let bytes = (count as usize)
            .checked_mul(width)
            .ok_or(Error::LargeLength(count as usize))?;
        self.input.read_into(&mut self.scratch, bytes)?;
        visitor.visit_bytes(&self.scratch)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    // Inside a list every element carries the same tag, so the element type is
    // already known: read it straight rather than round-tripping through an
    // `NbtTag`. A `Vec<i64>` off a LONG_ARRAY is thousands of elements per
    // section, and the tag check keeps a mistyped list falling back to `any`.
    define_in_list_number!(deserialize_i8, BYTE_ID, get_i8_be, visit_i8);
    define_in_list_number!(deserialize_i16, SHORT_ID, get_i16_be, visit_i16);
    define_in_list_number!(deserialize_i32, INT_ID, get_i32_be, visit_i32);
    define_in_list_number!(deserialize_i64, LONG_ID, get_i64_be, visit_i64);
    define_in_list_number!(deserialize_f32, FLOAT_ID, get_f32_be, visit_f32);
    define_in_list_number!(deserialize_f64, DOUBLE_ID, get_f64_be, visit_f64);

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let Some(tag) = self.tag_to_deserialize_stack else {
            return Err(Error::SerdeError("Ignoring nothing!".to_string()));
        };

        NbtTag::skip_data(&mut self.input, tag)?;
        visitor.visit_unit()
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let Some(tag_to_deserialize) = self.tag_to_deserialize_stack else {
            return Err(Error::SerdeError(
                "The top level must be a component (e.g. a struct)".to_string(),
            ));
        };

        match tag_to_deserialize {
            END_ID => Err(Error::SerdeError(
                "Trying to deserialize an END tag!".to_string(),
            )),
            LIST_ID | INT_ARRAY_ID | LONG_ARRAY_ID | BYTE_ARRAY_ID => {
                let list_type = match tag_to_deserialize {
                    LIST_ID => self.input.get_u8_be()?,
                    INT_ARRAY_ID => INT_ID,
                    LONG_ARRAY_ID => LONG_ID,
                    BYTE_ARRAY_ID => BYTE_ID,
                    _ => unreachable!(),
                };

                let remaining_values = self.input.get_i32_be()?;
                if remaining_values < 0 {
                    return Err(Error::NegativeLength(remaining_values));
                }

                let result = visitor.visit_seq(ListAccess {
                    de: self,
                    list_type,
                    remaining_values: remaining_values as usize,
                })?;
                Ok(result)
            }
            COMPOUND_ID => visitor.visit_map(CompoundAccess { de: self }),
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
                    _ => unreachable!(),
                };
                Ok(result)
            }
        }
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.in_list {
            let value = self.input.get_u8_be()?;
            visitor.visit_u8::<Error>(value)
        } else {
            Err(Error::UnsupportedType(
                "u8; NBT only supports signed values".to_string(),
            ))
        }
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
        if self.tag_to_deserialize_stack.unwrap() == BYTE_ID {
            let value = self.input.get_u8_be()?;
            if value != 0 {
                return visitor.visit_bool(true);
            }
        }
        visitor.visit_bool(false)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let variant = get_nbt_string(&mut self.input)?;
        visitor.visit_enum(variant.into_deserializer())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // None is not encoded, so no need for it
        visitor.visit_some(self)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let Some(tag_id) = self.tag_to_deserialize_stack {
            if tag_id != COMPOUND_ID {
                return Err(Error::SerdeError(format!(
                    "Trying to deserialize a map without a compound ID (id {tag_id})"
                )));
            }
        } else {
            let next_byte = self.input.get_u8_be()?;
            if next_byte != COMPOUND_ID {
                return Err(Error::NoRootCompound(next_byte));
            }

            if self.is_named {
                let length = self.input.get_u16_be()? as i64;
                self.input.skip_bytes(length)?;
            }
        }

        let value = visitor.visit_map(CompoundAccess { de: self })?;
        Ok(value)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_map(visitor)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let name = self.read_str()?;
        visitor.visit_str(&name)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

struct CompoundAccess<'a, R: Read + Seek> {
    de: &'a mut Deserializer<R>,
}

impl<'de, R: Read + Seek> MapAccess<'de> for CompoundAccess<'_, R> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let tag = self.de.input.get_u8_be()?;
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

struct MapKey<'a, R: Read + Seek> {
    de: &'a mut Deserializer<R>,
}

impl<'de, R: Read + Seek> de::Deserializer<'de> for MapKey<'_, R> {
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

struct ListAccess<'a, R: Read + Seek> {
    de: &'a mut Deserializer<R>,
    remaining_values: usize,
    list_type: u8,
}

impl<'de, R: Read + Seek> SeqAccess<'de> for ListAccess<'_, R> {
    type Error = Error;

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining_values)
    }

    fn next_element_seed<E: DeserializeSeed<'de>>(&mut self, seed: E) -> Result<Option<E::Value>> {
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

/// A list of compounds carries anything that is not one wrapped as
/// `{"": value}`, so a heterogeneous list still has a single element type.
/// Answers the wrapped value's tag, leaving the reader at its payload.
fn wrapper_payload<R: Read + Seek>(input: &mut NbtReadHelper<R>) -> Result<Option<u8>> {
    let start = input.position()?;
    let tag = input.get_u8_be()?;
    if tag != END_ID && input.get_u16_be()? == 0 {
        let payload = input.position()?;
        NbtTag::skip_data(input, tag)?;
        if input.get_u8_be()? == END_ID {
            input.seek_to(payload)?;
            return Ok(Some(tag));
        }
    }
    input.seek_to(start)?;
    Ok(None)
}
