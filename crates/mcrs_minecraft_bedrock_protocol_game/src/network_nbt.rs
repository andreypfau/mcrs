use alloc::vec::Vec;
use std::io::Cursor;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::{COMPOUND_ID, Nbt, NbtEncoding, NetworkLittleEndian};

/// An `nbt` field: one root compound in the network NBT form.
///
/// The root name is written empty; a decoded root name is dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkNbt(pub NbtCompound);

impl BedrockEncode for NetworkNbt {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let mut writer = WriteAdaptor::<_, NetworkLittleEndian>::with_encoding(w);
        writer.write_u8(COMPOUND_ID).map_err(wire_error)?;
        NetworkLittleEndian::write_string(&mut writer, "").map_err(wire_error)?;
        self.0.serialize_content(&mut writer).map_err(wire_error)
    }
}

impl<'a> BedrockDecode<'a> for NetworkNbt {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(*r);
        let nbt = Nbt::read_with(&mut NbtReadHelper::<_, NetworkLittleEndian>::with_encoding(
            &mut cursor,
        ))
        .map_err(wire_error)?;
        let read = usize::try_from(cursor.position()).map_err(|_| Error::LengthOverflow)?;
        *r = r.get(read..).ok_or(Error::UnexpectedEof)?;
        Ok(Self(nbt.root_tag))
    }
}

fn wire_error(error: mcrs_minecraft_nbt::Error) -> Error {
    match error {
        mcrs_minecraft_nbt::Error::Incomplete(_) => Error::UnexpectedEof,
        _ => Error::Nbt,
    }
}
