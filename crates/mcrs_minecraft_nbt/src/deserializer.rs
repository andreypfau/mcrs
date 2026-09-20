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
    depth: u32,
}

impl<R: Read + Seek> NbtReadHelper<R> {
    pub fn new(r: R) -> Self {
        Self {
            reader: r,
            depth: 0,
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
    // Vanilla's float and double tags fold -0.0 into +0.0 as they are read.
    pub fn get_f32_be(&mut self) -> Result<f32> {
        let mut buf = [0u8; 4];
        self.reader
            .read_exact(&mut buf)
            .map_err(Error::Incomplete)?;
        Ok(f32::from_be_bytes(buf) + 0.0)
    }

    pub fn get_f64_be(&mut self) -> Result<f64> {
        let mut buf = [0u8; 8];
        self.reader
            .read_exact(&mut buf)
            .map_err(Error::Incomplete)?;
        Ok(f64::from_be_bytes(buf) + 0.0)
    }

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
    _binary: crate::BinaryReadGuard,
}

impl<R: Read + Seek> Deserializer<R> {
    pub fn new(input: R, is_named: bool) -> Self {
        Deserializer {
            input: NbtReadHelper::new(input),
            tag_to_deserialize_stack: None,
            in_list: false,
            is_named,
            scratch: Vec::new(),
            _binary: crate::BinaryReadGuard::new(),
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

    /// A named (file) root must be a compound; a network root may be any tag
    /// but TAG_End.
    fn read_root(&mut self) -> Result<()> {
        if self.tag_to_deserialize_stack.is_some() {
            return Ok(());
        }
        let tag = self.input.get_u8_be()?;
        if tag == END_ID {
            return Err(Error::EndRoot);
        }
        if self.is_named {
            if tag != COMPOUND_ID {
                return Err(Error::NoRootCompound(tag));
            }
            let length = self.input.get_u16_be()? as i64;
            self.input.skip_bytes(length)?;
        }
        self.tag_to_deserialize_stack = Some(tag);
        Ok(())
    }
}

impl<R: Read + Seek> Deserializer<R> {
    fn visit_compound<'de, V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value> {
        self.input.push_depth()?;
        let result = visitor.visit_map(CompoundAccess { de: self });
        self.input.pop_depth();
        result
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
            _ => return self.deserialize_any(visitor),
        };
        visitor.visit_enum(ArrayAccess { de: self, variant })
    }

    // The whole payload goes to the visitor in one piece; read element-wise it
    // costs a visitor round trip each. A plain list still falls through to `any`.
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
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
        self.read_root()?;
        let tag = self.tag_to_deserialize_stack.unwrap();
        NbtTag::skip_data(&mut self.input, tag)?;
        visitor.visit_unit()
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
        let tag_to_deserialize = self.tag_to_deserialize_stack.unwrap();

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

                self.input.push_depth()?;
                let result = visitor.visit_seq(ListAccess {
                    de: self,
                    list_type,
                    remaining_values: remaining_values as usize,
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

    /// `NbtOps.getBooleanValue`: any numeric tag, true when non-zero.
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
        let tag = self.tag_to_deserialize_stack.unwrap();
        let value = match NbtTag::deserialize_data(&mut self.input, tag)? {
            NbtTag::Byte(v) => v != 0,
            NbtTag::Short(v) => v != 0,
            NbtTag::Int(v) => v != 0,
            NbtTag::Long(v) => v != 0,
            NbtTag::Float(v) => v != 0.0,
            NbtTag::Double(v) => v != 0.0,
            other => {
                return Err(Error::SerdeError(format!(
                    "invalid type: {}, expected a boolean",
                    other.get_type_id()
                )));
            }
        };
        visitor.visit_bool(value)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.read_root()?;
        let variant = get_nbt_string(&mut self.input)?;
        visitor.visit_enum(variant.into_deserializer())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // None is not encoded, so no need for it
        visitor.visit_some(self)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
        let tag_id = self.tag_to_deserialize_stack.unwrap();
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

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.read_root()?;
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

struct ArrayAccess<'a, R: Read + Seek> {
    de: &'a mut Deserializer<R>,
    variant: &'static str,
}

impl<'de, 'a, R: Read + Seek> de::EnumAccess<'de> for ArrayAccess<'a, R> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de, R: Read + Seek> de::VariantAccess<'de> for ArrayAccess<'_, R> {
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
