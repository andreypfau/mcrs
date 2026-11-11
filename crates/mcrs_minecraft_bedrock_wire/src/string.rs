use alloc::string::String;
use alloc::vec::Vec;
use core::str;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

impl BedrockEncode for str {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.as_bytes().encode(w)
    }
}

impl BedrockEncode for String {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.as_str().encode(w)
    }
}

impl<'a> BedrockDecode<'a> for &'a str {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let bytes: &'a [u8] = BedrockDecode::decode(r)?;
        str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }
}

impl<'a> BedrockDecode<'a> for String {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let text: &'a str = BedrockDecode::decode(r)?;
        Ok(String::from(text))
    }
}
