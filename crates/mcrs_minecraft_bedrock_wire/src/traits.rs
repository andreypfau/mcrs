//! Encoding and decoding traits for Bedrock wire types.

use crate::error::Error;
use alloc::vec::Vec;

/// Trait for types that can be serialized to Bedrock wire representation.
pub trait BedrockEncode {
    /// Writes this object to the provided buffer.
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error>;
}

/// Trait for types that can be deserialized from Bedrock wire representation.
pub trait BedrockDecode<'a>: Sized {
    /// Reads this object from the input slice, advancing the slice.
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error>;
}

impl BedrockEncode for crate::var_int::VarU32 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.write_to(w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for crate::var_int::VarU32 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Self::decode(r)
    }
}

impl BedrockEncode for crate::var_int::VarI32 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.write_to(w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for crate::var_int::VarI32 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Self::decode(r)
    }
}

impl BedrockEncode for crate::var_int::VarU64 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.write_to(w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for crate::var_int::VarU64 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Self::decode(r)
    }
}

impl BedrockEncode for crate::var_int::VarI64 {
    #[inline]
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.write_to(w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for crate::var_int::VarI64 {
    #[inline]
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Self::decode(r)
    }
}

impl<T: BedrockEncode> BedrockEncode for Vec<T> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let len = u32::try_from(self.len()).map_err(|_| Error::LengthOverflow)?;
        crate::var_int::VarU32(len).write_to(w);
        for item in self {
            item.encode(w)?;
        }
        Ok(())
    }
}

impl<'a, T: BedrockDecode<'a>> BedrockDecode<'a> for Vec<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let count = usize::try_from(crate::var_int::VarU32::decode(r)?.0)
            .map_err(|_| Error::LengthOverflow)?;
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            items.push(T::decode(r)?);
        }
        Ok(items)
    }
}

impl<T: BedrockEncode> BedrockEncode for Option<T> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Some(val) => {
                true.encode(w)?;
                val.encode(w)
            }
            None => false.encode(w),
        }
    }
}

impl<'a, T: BedrockDecode<'a>> BedrockDecode<'a> for Option<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let present = bool::decode(r)?;
        if present {
            Ok(Some(T::decode(r)?))
        } else {
            Ok(None)
        }
    }
}
