use std::io::{Read, Seek};

use crate::compound::NbtCompound;
use crate::deserializer::NbtReadHelper;
use crate::encoding::{BigEndian, NbtEncoding};
use crate::serializer::WriteAdaptor;
use crate::{COMPOUND_ID, Error};
use bytes::Bytes;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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

    pub fn read<R: Read + Seek>(reader: &mut NbtReadHelper<R, BigEndian>) -> Result<Nbt, Error> {
        Self::read_with::<R, BigEndian>(reader)
    }

    pub fn read_with<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
    ) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: get_nbt_string(reader)?,
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    /// Reads an NBT tag that doesn't contain the name of the root `Compound`.
    pub fn read_unnamed<R: Read + Seek>(
        reader: &mut NbtReadHelper<R, BigEndian>,
    ) -> Result<Nbt, Error> {
        Self::read_unnamed_with::<R, BigEndian>(reader)
    }

    /// Reads an NBT tag with specified encoding that doesn't contain the name of the root `Compound`.
    pub fn read_unnamed_with<R: Read + Seek, E: NbtEncoding>(
        reader: &mut NbtReadHelper<R, E>,
    ) -> Result<Nbt, Error> {
        let tag_type_id = reader.get_u8()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Nbt {
            name: String::new(),
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    pub fn write(&self) -> Result<Bytes, Error> {
        self.write_with::<BigEndian>()
    }

    pub fn write_with<E: NbtEncoding>(&self) -> Result<Bytes, Error> {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::<_, E>::with_encoding(&mut bytes);
        writer.write_u8(COMPOUND_ID)?;
        E::write_string(&mut writer, &self.name)?;
        self.root_tag.serialize_content(&mut writer)?;

        Ok(bytes.into())
    }

    /// Writes an NBT tag without a root `Compound` name.
    pub fn write_unnamed(&self) -> Result<Bytes, Error> {
        self.write_unnamed_with::<BigEndian>()
    }

    /// Writes an NBT tag with specified encoding without a root `Compound` name.
    pub fn write_unnamed_with<E: NbtEncoding>(&self) -> Result<Bytes, Error> {
        let mut bytes = Vec::new();
        let mut writer = WriteAdaptor::<_, E>::with_encoding(&mut bytes);

        writer.write_u8(COMPOUND_ID)?;
        self.root_tag.serialize_content(&mut writer)?;

        Ok(bytes.into())
    }
}

pub fn get_nbt_string<R: Read + Seek, E: NbtEncoding>(
    bytes: &mut NbtReadHelper<R, E>,
) -> Result<String, Error> {
    let len = E::read_string_length(&mut bytes.reader)?;
    let string_bytes = bytes.read_boxed_slice(len)?;
    Ok(E::decode_string(&string_bytes)?.into_owned())
}
