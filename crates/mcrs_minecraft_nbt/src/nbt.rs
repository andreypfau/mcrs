use std::io::{Read, Seek};

use crate::compound::NbtCompound;
use crate::deserializer::NbtReadHelper;
use crate::serializer::WriteAdaptor;
use crate::tag::NbtTag;
use crate::{COMPOUND_ID, Error};
use bytes::Bytes;

#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Nbt {
    pub name: String,
    pub root_tag: NbtCompound,
}

impl Nbt {
    pub fn new(name: String, tag: NbtCompound) -> Self {
        Nbt {
            name,
            root_tag: tag,
        }
    }

    pub fn read<R: Read + Seek>(reader: &mut NbtReadHelper<R>) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8_be()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: get_nbt_string(reader)?,
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    /// Reads an NBT tag that doesn't contain the name of the root `Compound`.
    pub fn read_unnamed<R: Read + Seek>(reader: &mut NbtReadHelper<R>) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8_be()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: String::new(),
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    pub fn write(&self) -> Result<Bytes, Error> {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::new(&mut bytes);
        writer.write_u8_be(COMPOUND_ID)?;
        NbtTag::String(self.name.to_string()).serialize_data(&mut writer)?;
        self.root_tag.serialize_content(&mut writer)?;

        Ok(bytes.into())
    }

    /// Writes an NBT tag without a root `Compound` name.
    pub fn write_unnamed(&self) -> Result<Bytes, Error> {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::new(&mut bytes);

        writer.write_u8_be(COMPOUND_ID)?;
        self.root_tag.serialize_content(&mut writer)?;

        Ok(bytes.into())
    }
}

pub fn get_nbt_string<R: Read + Seek>(bytes: &mut NbtReadHelper<R>) -> Result<String, Error> {
    let len = bytes.get_u16_be()? as usize;
    let string_bytes = bytes.read_boxed_slice(len)?;
    let string = cesu8::from_java_cesu8(&string_bytes).map_err(|_| Error::Cesu8DecodingError)?;
    Ok(string.to_string())
}
