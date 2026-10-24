use std::io::{Cursor, Write};

use mcrs_minecraft_wire::{Decode, Encode, Error, ErrorKind, Result};

use crate::Nbt;
use crate::compound::NbtCompound;
use crate::deserializer::NbtReadHelper;
use crate::serializer::WriteAdaptor;
use crate::tag::NbtTag;

impl Encode for NbtCompound {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let mut writer = WriteAdaptor::new(&mut w);
        writer
            .write_u8_be(crate::COMPOUND_ID)
            .map_err(Error::other)?;
        self.serialize_content(&mut writer).map_err(Error::other)
    }
}

impl Decode<'_> for NbtCompound {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(*r);
        let nbt = Nbt::read_unnamed(&mut NbtReadHelper::new(&mut cursor)).map_err(Error::other)?;
        *r = &r[cursor.position() as usize..];
        Ok(nbt.root_tag)
    }
}

impl Encode for NbtTag {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.serialize(&mut WriteAdaptor::new(&mut w))
            .map_err(Error::other)
    }
}

impl Decode<'_> for NbtTag {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(*r);
        let tag =
            NbtTag::deserialize(&mut NbtReadHelper::new(&mut cursor)).map_err(Error::other)?;
        *r = &r[cursor.position() as usize..];
        Ok(tag)
    }
}

/// Writes a serde value as one network NBT tag: an unnamed root of whatever type the value
/// serializes to.
pub fn encode_wire<T: serde::Serialize>(value: &T, w: impl Write) -> Result<()> {
    crate::to_bytes_unnamed(value, w).map_err(Error::other)
}

pub type WireDeserializer<'x, 'y> = crate::deserializer::Deserializer<&'x mut Cursor<&'y [u8]>>;

/// Reads one network NBT tag through `read`, and advances `r` past it.
pub fn read_wire<T>(
    r: &mut &[u8],
    read: impl FnOnce(&mut WireDeserializer<'_, '_>) -> Result<T>,
) -> Result<T> {
    match r.first() {
        None => return Err(ErrorKind::UnexpectedEnd.into()),
        Some(&crate::END_ID) => {
            return Err(Error::invalid("a network NBT tag must not be TAG_End"));
        }
        Some(_) => {}
    }
    let mut cursor = Cursor::new(*r);
    let mut d = crate::deserializer::Deserializer::new(&mut cursor, false);
    let value = read(&mut d)?;
    *r = &r[cursor.position() as usize..];
    Ok(value)
}

pub fn decode_wire<T: serde::de::DeserializeOwned>(r: &mut &[u8]) -> Result<T> {
    read_wire(r, |d| T::deserialize(&mut *d).map_err(Error::other))
}
