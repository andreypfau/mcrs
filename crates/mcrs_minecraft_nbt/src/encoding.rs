use std::borrow::Cow;
use std::fmt::Debug;
use std::io::{Read, Write};

use crate::{BYTE_ID, DOUBLE_ID, Error, FLOAT_ID, INT_ID, LONG_ID, SHORT_ID};

/// The byte-level form of NBT: byte order, length prefixes, string encoding
/// and whether integers are fixed-width or zigzag varints.
///
/// The provided methods are the forms that more than one encoding shares.
pub trait NbtEncoding: Debug {
    /// The payload size of a primitive tag, when this encoding fixes it.
    #[inline]
    fn fixed_size(tag_id: u8) -> Option<i64> {
        match tag_id {
            BYTE_ID => Some(1),
            SHORT_ID => Some(2),
            INT_ID | FLOAT_ID => Some(4),
            LONG_ID | DOUBLE_ID => Some(8),
            _ => None,
        }
    }

    fn write_i16<W: Write>(w: &mut W, value: i16) -> Result<(), Error>;
    fn write_u16<W: Write>(w: &mut W, value: u16) -> Result<(), Error>;
    fn write_i32<W: Write>(w: &mut W, value: i32) -> Result<(), Error>;
    fn write_i64<W: Write>(w: &mut W, value: i64) -> Result<(), Error>;
    fn write_f32<W: Write>(w: &mut W, value: f32) -> Result<(), Error>;
    fn write_f64<W: Write>(w: &mut W, value: f64) -> Result<(), Error>;

    fn read_i16<R: Read>(r: &mut R) -> Result<i16, Error>;
    fn read_u16<R: Read>(r: &mut R) -> Result<u16, Error>;
    fn read_i32<R: Read>(r: &mut R) -> Result<i32, Error>;
    fn read_i64<R: Read>(r: &mut R) -> Result<i64, Error>;
    fn read_f32<R: Read>(r: &mut R) -> Result<f32, Error>;
    fn read_f64<R: Read>(r: &mut R) -> Result<f64, Error>;

    #[inline]
    fn write_string_length<W: Write>(w: &mut W, len: usize) -> Result<(), Error> {
        Self::write_u16(w, u16::try_from(len).map_err(|_| Error::LargeLength(len))?)
    }

    #[inline]
    fn read_string_length<R: Read>(r: &mut R) -> Result<usize, Error> {
        Ok(usize::from(Self::read_u16(r)?))
    }

    #[inline]
    fn write_collection_length<W: Write>(w: &mut W, len: usize) -> Result<(), Error> {
        Self::write_i32(w, i32::try_from(len).map_err(|_| Error::LargeLength(len))?)
    }

    #[inline]
    fn read_collection_length<R: Read>(r: &mut R) -> Result<usize, Error> {
        let len = Self::read_i32(r)?;
        usize::try_from(len).map_err(|_| Error::NegativeLength(len))
    }

    fn write_string<W: Write>(w: &mut W, string: &str) -> Result<(), Error> {
        Self::write_string_length(w, string.len())?;
        w.write_all(string.as_bytes()).map_err(Error::Incomplete)
    }

    #[inline]
    fn decode_string(bytes: &[u8]) -> Result<Cow<'_, str>, Error> {
        std::str::from_utf8(bytes)
            .map(Cow::Borrowed)
            .map_err(|_| Error::InvalidUtf8)
    }
}

/// Big-endian NBT with modified UTF-8 strings: Java Edition.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BigEndian;

/// Little-endian NBT with UTF-8 strings: Bedrock Edition world storage.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LittleEndian;

/// Little-endian NBT whose ints, longs and lengths are varints: Bedrock
/// Edition packets.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkLittleEndian;

macro_rules! fixed_width {
    ($to_bytes:ident, $from_bytes:ident: $($write:ident $read:ident $ty:ty),+ $(,)?) => {$(
        #[inline]
        fn $write<W: Write>(w: &mut W, value: $ty) -> Result<(), Error> {
            w.write_all(&value.$to_bytes()).map_err(Error::Incomplete)
        }

        #[inline]
        fn $read<R: Read>(r: &mut R) -> Result<$ty, Error> {
            let mut bytes = [0u8; size_of::<$ty>()];
            r.read_exact(&mut bytes).map_err(Error::Incomplete)?;
            Ok(<$ty>::$from_bytes(bytes))
        }
    )+};
}

impl NbtEncoding for BigEndian {
    fixed_width!(to_be_bytes, from_be_bytes:
        write_i16 read_i16 i16,
        write_u16 read_u16 u16,
        write_i32 read_i32 i32,
        write_i64 read_i64 i64,
        write_f32 read_f32 f32,
        write_f64 read_f64 f64,
    );

    fn write_string<W: Write>(w: &mut W, string: &str) -> Result<(), Error> {
        let bytes = cesu8::to_java_cesu8(string);
        Self::write_string_length(w, bytes.len())?;
        w.write_all(&bytes).map_err(Error::Incomplete)
    }

    #[inline]
    fn decode_string(bytes: &[u8]) -> Result<Cow<'_, str>, Error> {
        cesu8::from_java_cesu8(bytes).map_err(|_| Error::Cesu8DecodingError)
    }
}

impl NbtEncoding for LittleEndian {
    fixed_width!(to_le_bytes, from_le_bytes:
        write_i16 read_i16 i16,
        write_u16 read_u16 u16,
        write_i32 read_i32 i32,
        write_i64 read_i64 i64,
        write_f32 read_f32 f32,
        write_f64 read_f64 f64,
    );
}

impl NbtEncoding for NetworkLittleEndian {
    #[inline]
    fn fixed_size(tag_id: u8) -> Option<i64> {
        match tag_id {
            BYTE_ID => Some(1),
            SHORT_ID => Some(2),
            FLOAT_ID => Some(4),
            DOUBLE_ID => Some(8),
            _ => None,
        }
    }

    fixed_width!(to_le_bytes, from_le_bytes:
        write_i16 read_i16 i16,
        write_u16 read_u16 u16,
        write_f32 read_f32 f32,
        write_f64 read_f64 f64,
    );

    #[inline]
    fn write_i32<W: Write>(w: &mut W, value: i32) -> Result<(), Error> {
        let zigzag = ((value as u32) << 1) ^ ((value >> 31) as u32);
        write_varint(w, u64::from(zigzag))
    }

    #[inline]
    fn read_i32<R: Read>(r: &mut R) -> Result<i32, Error> {
        let zigzag = read_varint(r, u32::BITS)? as u32;
        Ok(((zigzag >> 1) as i32) ^ -((zigzag & 1) as i32))
    }

    #[inline]
    fn write_i64<W: Write>(w: &mut W, value: i64) -> Result<(), Error> {
        write_varint(w, ((value as u64) << 1) ^ ((value >> 63) as u64))
    }

    #[inline]
    fn read_i64<R: Read>(r: &mut R) -> Result<i64, Error> {
        let zigzag = read_varint(r, u64::BITS)?;
        Ok(((zigzag >> 1) as i64) ^ -((zigzag & 1) as i64))
    }

    #[inline]
    fn write_string_length<W: Write>(w: &mut W, len: usize) -> Result<(), Error> {
        let len = u32::try_from(len).map_err(|_| Error::LargeLength(len))?;
        write_varint(w, u64::from(len))
    }

    #[inline]
    fn read_string_length<R: Read>(r: &mut R) -> Result<usize, Error> {
        let len = read_varint(r, u32::BITS)?;
        usize::try_from(len).map_err(|_| Error::MalformedVarInt)
    }
}

fn write_varint<W: Write>(w: &mut W, mut value: u64) -> Result<(), Error> {
    while value >= 0x80 {
        w.write_all(&[value as u8 | 0x80])
            .map_err(Error::Incomplete)?;
        value >>= 7;
    }
    w.write_all(&[value as u8]).map_err(Error::Incomplete)
}

/// Reads an unsigned varint of at most `bits` bits. A group that carries a
/// bit past `bits`, or a continuation past the last group, is malformed.
fn read_varint<R: Read>(r: &mut R, bits: u32) -> Result<u64, Error> {
    let mut value = 0u64;
    let mut shift = 0;
    while shift < bits {
        let mut byte = [0u8];
        r.read_exact(&mut byte).map_err(Error::Incomplete)?;
        let [byte] = byte;
        let group = u64::from(byte & 0x7F);
        if shift + 7 > bits && group >> (bits - shift) != 0 {
            return Err(Error::MalformedVarInt);
        }
        value |= group << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
    Err(Error::MalformedVarInt)
}
