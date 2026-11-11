use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

/// A big-endian `i32`, the one integer that Bedrock does not write little-endian.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BeI32(pub i32);

impl BedrockEncode for BeI32 {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        w.extend_from_slice(&self.0.to_be_bytes());
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for BeI32 {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let Some((bytes, rest)) = r.split_first_chunk::<4>() else {
            return Err(Error::UnexpectedEof);
        };
        *r = rest;
        Ok(Self(i32::from_be_bytes(*bytes)))
    }
}
