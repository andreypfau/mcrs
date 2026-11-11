use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

impl<T: BedrockEncode> BedrockEncode for Option<T> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.is_some().encode(w)?;
        match self {
            Some(value) => value.encode(w),
            None => Ok(()),
        }
    }
}

impl<'a, T: BedrockDecode<'a>> BedrockDecode<'a> for Option<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        if bool::decode(r)? {
            T::decode(r).map(Some)
        } else {
            Ok(None)
        }
    }
}
