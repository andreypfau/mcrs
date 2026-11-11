use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

macro_rules! unsigned_var_int {
    ($name:ident, $int:ty, $max_len:literal, $last_byte_mask:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $int);

        impl BedrockEncode for $name {
            fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
                let mut bytes = [0u8; $max_len];
                let mut value = self.0;
                let mut len = 0;
                for byte in &mut bytes {
                    *byte = (value & 0x7F) as u8;
                    value >>= 7;
                    len += 1;
                    if value == 0 {
                        break;
                    }
                    *byte |= 0x80;
                }
                w.extend_from_slice(bytes.get(..len).unwrap_or(&bytes));
                Ok(())
            }
        }

        impl<'a> BedrockDecode<'a> for $name {
            fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
                let mut value: $int = 0;
                for index in 0..$max_len {
                    let Some((&byte, rest)) = r.split_first() else {
                        return Err(Error::UnexpectedEof);
                    };
                    *r = rest;
                    if index == $max_len - 1 && byte & $last_byte_mask != 0 {
                        return Err(Error::VarIntOverflow);
                    }
                    value |= <$int>::from(byte & 0x7F) << (index * 7);
                    if byte & 0x80 == 0 {
                        return Ok(Self(value));
                    }
                }
                Err(Error::VarIntOverflow)
            }
        }
    };
}

macro_rules! zigzag_var_int {
    ($name:ident, $int:ty, $unsigned:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $int);

        impl BedrockEncode for $name {
            fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
                let zigzag = (self.0 << 1) ^ (self.0 >> (<$int>::BITS - 1));
                $unsigned(zigzag.cast_unsigned()).encode(w)
            }
        }

        impl<'a> BedrockDecode<'a> for $name {
            fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
                let $unsigned(zigzag) = $unsigned::decode(r)?;
                Ok(Self(
                    (zigzag >> 1).cast_signed() ^ -(zigzag & 1).cast_signed(),
                ))
            }
        }
    };
}

// The last byte keeps only the bits that are left of the integer: 32 - 28 = 4, 64 - 63 = 1.
unsigned_var_int!(
    VarU32,
    u32,
    5,
    0xF0,
    "A `u32` in unsigned LEB128, 1 to 5 bytes."
);
unsigned_var_int!(
    VarU64,
    u64,
    10,
    0xFE,
    "A `u64` in unsigned LEB128, 1 to 10 bytes."
);
zigzag_var_int!(
    VarI32,
    i32,
    VarU32,
    "An `i32` in zigzag LEB128, 1 to 5 bytes."
);
zigzag_var_int!(
    VarI64,
    i64,
    VarU64,
    "An `i64` in zigzag LEB128, 1 to 10 bytes."
);
