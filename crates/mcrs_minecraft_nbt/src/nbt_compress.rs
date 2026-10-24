use crate::deserializer::NbtReadHelper;
use crate::{Error, Nbt, NbtCompound, deserializer, serializer};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use std::io::{Cursor, Read, Seek, Write};

pub fn read_gzip_compound_tag(input: impl Read + Seek) -> Result<NbtCompound, Error> {
    let mut decoder = GzDecoder::new(input);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(Error::Incomplete)?;
    let mut reader = NbtReadHelper::new(Cursor::new(buf));
    let nbt = Nbt::read(&mut reader)?;
    Ok(nbt.root_tag)
}

pub fn write_gzip_compound_tag_to_bytes(compound: &NbtCompound) -> Result<Vec<u8>, Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&Nbt::new(String::new(), compound.clone()).write()?)
        .map_err(Error::Incomplete)?;
    encoder.finish().map_err(Error::Incomplete)
}

pub fn from_gzip_bytes<'a, T: serde::Deserialize<'a>, R: Read>(input: R) -> Result<T, Error> {
    let mut decoder = GzDecoder::new(input);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(Error::Incomplete)?;
    deserializer::from_bytes(Cursor::new(buf))
}

pub fn to_gzip_bytes_vec<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    serializer::to_bytes(value, GzEncoder::new(&mut buffer, Compression::default()))?;
    Ok(buffer)
}
