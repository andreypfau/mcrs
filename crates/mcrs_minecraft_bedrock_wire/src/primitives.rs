//! Little-endian primitive implementations for Bedrock Edition wire encoding.

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use alloc::vec::Vec;

macro_rules! impl_le_primitive {
    ($type:ty, $size:expr) => {
        impl BedrockEncode for $type {
            #[inline]
            fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
                w.extend_from_slice(&self.to_le_bytes());
                Ok(())
            }
        }

        impl<'a> BedrockDecode<'a> for $type {
            #[inline]
            fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
                let Some((bytes, rest)) = r.split_first_chunk::<$size>() else {
                    return Err(Error::UnexpectedEof);
                };
                *r = rest;
                Ok(<$type>::from_le_bytes(*bytes))
            }
        }
    };
}

impl_le_primitive!(u16, 2);
impl_le_primitive!(i16, 2);
impl_le_primitive!(u32, 4);
impl_le_primitive!(i32, 4);
impl_le_primitive!(u64, 8);
impl_le_primitive!(i64, 8);
impl_le_primitive!(u128, 16);
impl_le_primitive!(i128, 16);
impl_le_primitive!(f32, 4);
impl_le_primitive!(f64, 8);

impl BedrockEncode for u8 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.push(*self);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for u8 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let Some((&b, rest)) = r.split_first() else {
            return Err(Error::UnexpectedEof);
        };
        *r = rest;
        Ok(b)
    }
}

impl BedrockEncode for i8 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.push(*self as u8);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for i8 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let b = u8::decode(r)?;
        Ok(b as i8)
    }
}

impl BedrockEncode for bool {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.push(if *self { 1 } else { 0 });
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for bool {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let b = u8::decode(r)?;
        match b {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidBool),
        }
    }
}
