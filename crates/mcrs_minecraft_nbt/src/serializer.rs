use serde::Serialize;
use std::io::Write;

use crate::Error;
use crate::tag::NbtTag;
use crate::tag_serializer::to_nbt_tag;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct WriteAdaptor<W: Write> {
    writer: W,
}

impl<W: Write> WriteAdaptor<W> {
    pub fn new(w: W) -> Self {
        Self { writer: w }
    }
}

macro_rules! write_number_be {
    ($name:ident, $type:ty) => {
        pub fn $name(&mut self, value: $type) -> Result<()> {
            let buf = value.to_be_bytes();
            self.writer.write_all(&buf).map_err(Error::Incomplete)?;
            Ok(())
        }
    };
}

impl<W: Write> WriteAdaptor<W> {
    write_number_be!(write_u8_be, u8);
    write_number_be!(write_i8_be, i8);
    write_number_be!(write_u16_be, u16);
    write_number_be!(write_i16_be, i16);
    write_number_be!(write_i32_be, i32);
    write_number_be!(write_i64_be, i64);
    // Vanilla's float and double tags fold -0.0 into +0.0 as they are made.
    pub fn write_f32_be(&mut self, value: f32) -> Result<()> {
        self.writer
            .write_all(&(value + 0.0).to_be_bytes())
            .map_err(Error::Incomplete)
    }

    pub fn write_f64_be(&mut self, value: f64) -> Result<()> {
        self.writer
            .write_all(&(value + 0.0).to_be_bytes())
            .map_err(Error::Incomplete)
    }

    pub fn write_slice(&mut self, value: &[u8]) -> Result<()> {
        self.writer.write_all(value).map_err(Error::Incomplete)?;
        Ok(())
    }
}

fn write_root<T: Serialize>(value: &T, name: Option<String>, w: impl Write) -> Result<()> {
    let tag = to_nbt_tag(value)?;
    match tag {
        NbtTag::End => return Err(Error::EndRoot),
        NbtTag::Compound(_) => {}
        _ if name.is_some() => return Err(Error::NoRootCompound(tag.get_type_id())),
        _ => {}
    }
    let mut w = WriteAdaptor::new(w);
    w.write_u8_be(tag.get_type_id())?;
    if let Some(name) = name {
        NbtTag::String(name).serialize_data(&mut w)?;
    }
    tag.serialize_data(&mut w)
}

/// Serializes struct using Serde Serializer to unnamed (network) NBT
pub fn to_bytes_unnamed<T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    write_root(value, None, w)
}

/// Serializes struct using Serde Serializer to normal NBT
pub fn to_bytes_named<T: Serialize>(value: &T, name: String, w: impl Write) -> Result<()> {
    write_root(value, Some(name), w)
}

pub fn to_bytes<T: Serialize>(value: &T, w: impl Write) -> Result<()> {
    to_bytes_named(value, String::new(), w)
}
