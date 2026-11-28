use alloc::vec::Vec;

use crate::error::Error;

pub trait BedrockEncode {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error>;

    fn encode_slice(items: &[Self], w: &mut Vec<u8>) -> Result<(), Error>
    where
        Self: Sized,
    {
        for item in items {
            item.encode(w)?;
        }
        Ok(())
    }
}

pub trait BedrockDecode<'a>: Sized {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error>;
}

impl<T: BedrockEncode + ?Sized> BedrockEncode for &T {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        (**self).encode(w)
    }
}
