//! Bedrock Edition string encoding (VarU32 length prefix followed by UTF-8 bytes).

use alloc::string::String;
use alloc::vec::Vec;
use core::str;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::VarU32;

impl BedrockEncode for str {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let len = u32::try_from(self.len()).map_err(|_| Error::LengthOverflow)?;
        VarU32(len).write_to(w);
        w.extend_from_slice(self.as_bytes());
        Ok(())
    }
}

impl BedrockEncode for String {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.as_str().encode(w)
    }
}

impl<'a> BedrockDecode<'a> for &'a str {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let len = usize::try_from(VarU32::decode(r)?.0).map_err(|_| Error::LengthOverflow)?;
        if r.len() < len {
            return Err(Error::UnexpectedEof);
        }
        let (bytes, rest) = r.split_at(len);
        *r = rest;
        str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }
}

impl<'a> BedrockDecode<'a> for String {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let s: &'a str = BedrockDecode::decode(r)?;
        Ok(s.into())
    }
}
