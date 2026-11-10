//! Variable-length integer implementations with Bedrock ZigZag encoding.

use crate::error::Error;
use alloc::vec::Vec;

/// Unsigned 32-bit variable-length integer (LEB128).
///
/// Encodes in 1 to 5 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarU32(pub u32);

impl VarU32 {
    /// Writes this unsigned varint into the output buffer.
    pub fn write_to(self, w: &mut Vec<u8>) {
        let mut val = self.0;
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
                w.push(byte);
            } else {
                w.push(byte);
                break;
            }
        }
    }

    /// Reads an unsigned varint from the input slice, advancing the slice.
    pub fn decode(r: &mut &[u8]) -> Result<Self, Error> {
        let mut result = 0u32;
        let mut shift = 0;
        for i in 0..5 {
            let Some((&byte, rest)) = r.split_first() else {
                return Err(Error::UnexpectedEof);
            };
            *r = rest;

            if i == 4 && (byte & 0xF0) != 0 {
                return Err(Error::VarIntOverflow);
            }

            result |= ((byte & 0x7F) as u32) << shift;
            if (byte & 0x80) == 0 {
                return Ok(Self(result));
            }
            shift += 7;
        }
        Err(Error::InvalidVarInt)
    }

    /// Returns the number of bytes this value takes when encoded.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        let val = self.0;
        if val < (1 << 7) {
            1
        } else if val < (1 << 14) {
            2
        } else if val < (1 << 21) {
            3
        } else if val < (1 << 28) {
            4
        } else {
            5
        }
    }
}

/// Signed 32-bit variable-length integer with ZigZag encoding.
///
/// In Bedrock Edition, signed 32-bit integers use ZigZag encoding:
/// `u = (n << 1) ^ (n >> 31)`, encoded as an unsigned varint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarI32(pub i32);

impl VarI32 {
    /// ZigZag encodes an `i32` to a `u32`.
    #[must_use]
    pub const fn zigzag_encode(val: i32) -> u32 {
        ((val << 1) ^ (val >> 31)) as u32
    }

    /// ZigZag decodes a `u32` to an `i32`.
    #[must_use]
    pub const fn zigzag_decode(val: u32) -> i32 {
        ((val >> 1) as i32) ^ (-((val & 1) as i32))
    }

    /// Writes this signed varint into the output buffer.
    pub fn write_to(self, w: &mut Vec<u8>) {
        VarU32(Self::zigzag_encode(self.0)).write_to(w);
    }

    /// Reads a signed varint from the input slice, advancing the slice.
    pub fn decode(r: &mut &[u8]) -> Result<Self, Error> {
        let unsigned = VarU32::decode(r)?;
        Ok(Self(Self::zigzag_decode(unsigned.0)))
    }

    /// Returns the number of bytes this value takes when encoded.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        VarU32(Self::zigzag_encode(self.0)).encoded_len()
    }
}

/// Unsigned 64-bit variable-length integer (LEB128).
///
/// Encodes in 1 to 10 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarU64(pub u64);

impl VarU64 {
    /// Writes this unsigned varlong into the output buffer.
    pub fn write_to(self, w: &mut Vec<u8>) {
        let mut val = self.0;
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
                w.push(byte);
            } else {
                w.push(byte);
                break;
            }
        }
    }

    /// Reads an unsigned varlong from the input slice, advancing the slice.
    pub fn decode(r: &mut &[u8]) -> Result<Self, Error> {
        let mut result = 0u64;
        let mut shift = 0;
        for i in 0..10 {
            let Some((&byte, rest)) = r.split_first() else {
                return Err(Error::UnexpectedEof);
            };
            *r = rest;

            if i == 9 && (byte & 0xFE) != 0 {
                return Err(Error::VarIntOverflow);
            }

            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                return Ok(Self(result));
            }
            shift += 7;
        }
        Err(Error::InvalidVarInt)
    }

    /// Returns the number of bytes this value takes when encoded.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        let val = self.0;
        if val < (1 << 7) {
            1
        } else if val < (1 << 14) {
            2
        } else if val < (1 << 21) {
            3
        } else if val < (1 << 28) {
            4
        } else if val < (1 << 35) {
            5
        } else if val < (1 << 42) {
            6
        } else if val < (1 << 49) {
            7
        } else if val < (1 << 56) {
            8
        } else if val < (1 << 63) {
            9
        } else {
            10
        }
    }
}

/// Signed 64-bit variable-length integer with ZigZag encoding.
///
/// In Bedrock Edition, signed 64-bit integers use ZigZag encoding:
/// `u = (n << 1) ^ (n >> 63)`, encoded as an unsigned varint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VarI64(pub i64);

impl VarI64 {
    /// ZigZag encodes an `i64` to a `u64`.
    #[must_use]
    pub const fn zigzag_encode(val: i64) -> u64 {
        ((val << 1) ^ (val >> 63)) as u64
    }

    /// ZigZag decodes a `u64` to an `i64`.
    #[must_use]
    pub const fn zigzag_decode(val: u64) -> i64 {
        ((val >> 1) as i64) ^ (-((val & 1) as i64))
    }

    /// Writes this signed varlong into the output buffer.
    pub fn write_to(self, w: &mut Vec<u8>) {
        VarU64(Self::zigzag_encode(self.0)).write_to(w);
    }

    /// Reads a signed varlong from the input slice, advancing the slice.
    pub fn decode(r: &mut &[u8]) -> Result<Self, Error> {
        let unsigned = VarU64::decode(r)?;
        Ok(Self(Self::zigzag_decode(unsigned.0)))
    }

    /// Returns the number of bytes this value takes when encoded.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        VarU64(Self::zigzag_encode(self.0)).encoded_len()
    }
}
