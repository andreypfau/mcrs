use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

macro_rules! little_endian {
    ($int:ty, $size:literal) => {
        impl BedrockEncode for $int {
            fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
                w.extend_from_slice(&self.to_le_bytes());
                Ok(())
            }
        }

        impl<'a> BedrockDecode<'a> for $int {
            fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
                let Some((bytes, rest)) = r.split_first_chunk::<$size>() else {
                    return Err(Error::UnexpectedEof);
                };
                *r = rest;
                Ok(<$int>::from_le_bytes(*bytes))
            }
        }
    };
}

little_endian!(u16, 2);
little_endian!(i16, 2);
little_endian!(u32, 4);
little_endian!(i32, 4);
little_endian!(u64, 8);
little_endian!(i64, 8);
little_endian!(f32, 4);

impl BedrockEncode for u8 {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.push(*self);
        Ok(())
    }

    fn encode_slice(items: &[Self], w: &mut Vec<u8>) -> Result<(), Error> {
        w.extend_from_slice(items);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for u8 {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let Some((&byte, rest)) = r.split_first() else {
            return Err(Error::UnexpectedEof);
        };
        *r = rest;
        Ok(byte)
    }
}

impl BedrockEncode for bool {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.push(u8::from(*self));
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for bool {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(u8::decode(r)? != 0)
    }
}
