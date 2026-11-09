use std::io::Write;
use std::marker::PhantomData;

use serde::Serialize;

use crate::Error;
use crate::encoding::{BigEndian, NbtEncoding};
use crate::tag::NbtTag;
use crate::tag_serializer::to_nbt_tag;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct WriteAdaptor<W: Write, E: NbtEncoding = BigEndian> {
    writer: W,
    _encoding: PhantomData<E>,
}

impl<W: Write> WriteAdaptor<W, BigEndian> {
    pub fn new(w: W) -> Self {
        Self::with_encoding(w)
    }
}

impl<W: Write, E: NbtEncoding> WriteAdaptor<W, E> {
    pub fn with_encoding(w: W) -> Self {
        Self {
            writer: w,
            _encoding: PhantomData,
        }
    }

    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    #[inline]
    pub fn write_u8(&mut self, value: u8) -> Result<()> {
        self.writer.write_all(&[value]).map_err(Error::Incomplete)
    }

    #[inline]
    pub fn write_i8(&mut self, value: i8) -> Result<()> {
        self.writer
            .write_all(&[value as u8])
            .map_err(Error::Incomplete)
    }

    #[inline]
    pub fn write_u16(&mut self, value: u16) -> Result<()> {
        E::write_u16(&mut self.writer, value)
    }

    #[inline]
    pub fn write_i16(&mut self, value: i16) -> Result<()> {
        E::write_i16(&mut self.writer, value)
    }

    #[inline]
    pub fn write_i32(&mut self, value: i32) -> Result<()> {
        E::write_i32(&mut self.writer, value)
    }

    #[inline]
    pub fn write_i64(&mut self, value: i64) -> Result<()> {
        E::write_i64(&mut self.writer, value)
    }

    // Vanilla's float and double tags fold -0.0 into +0.0 as they are made.
    #[inline]
    pub fn write_f32(&mut self, value: f32) -> Result<()> {
        E::write_f32(&mut self.writer, value + 0.0)
    }

    #[inline]
    pub fn write_f64(&mut self, value: f64) -> Result<()> {
        E::write_f64(&mut self.writer, value + 0.0)
    }

    #[inline]
    pub fn write_slice(&mut self, value: &[u8]) -> Result<()> {
        self.writer.write_all(value).map_err(Error::Incomplete)?;
        Ok(())
    }
}

impl<W: Write, E: NbtEncoding> Write for WriteAdaptor<W, E> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn write_root<E: NbtEncoding, T: Serialize>(
    value: &T,
    name: Option<String>,
    w: impl Write,
) -> Result<()> {
    let tag = to_nbt_tag(value)?;
    match tag {
        NbtTag::End => return Err(Error::EndRoot),
        NbtTag::Compound(_) => {}
        _ if name.is_some() => return Err(Error::NoRootCompound(tag.get_type_id())),
        _ => {}
    }
    let mut w = WriteAdaptor::<_, E>::with_encoding(w);
    w.write_u8(tag.get_type_id())?;
    if let Some(name) = name {
        E::write_string(&mut w, &name)?;
    }
    tag.serialize_data(&mut w)
}

/// Serializes struct using Serde Serializer to unnamed (network) NBT with standard BigEndian encoding
pub fn to_bytes_unnamed<T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    to_bytes_unnamed_with::<BigEndian, T>(value, w)
}

/// Serializes struct using Serde Serializer to unnamed NBT with specified encoding
pub fn to_bytes_unnamed_with<E: NbtEncoding, T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    write_root::<E, T>(value, None, w)
}

/// Serializes struct using Serde Serializer to normal named NBT with standard BigEndian encoding
pub fn to_bytes_named<T: Serialize>(value: &T, name: String, w: impl Write) -> Result<()> {
    to_bytes_named_with::<BigEndian, T>(value, name, w)
}

/// Serializes struct using Serde Serializer to normal named NBT with specified encoding
pub fn to_bytes_named_with<E: NbtEncoding, T: Serialize>(
    value: &T,
    name: String,
    w: impl Write,
) -> Result<()> {
    write_root::<E, T>(value, Some(name), w)
}

/// Serializes struct using Serde Serializer with standard BigEndian encoding and empty root name
pub fn to_bytes<T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    to_bytes_with::<BigEndian, T>(value, w)
}

/// Serializes struct using Serde Serializer with specified encoding and empty root name
pub fn to_bytes_with<E: NbtEncoding, T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    to_bytes_named_with::<E, T>(value, String::new(), w)
}
