use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

pub fn to_bytes<T: BedrockEncode + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    value.encode(&mut buffer)?;
    Ok(buffer)
}

pub fn from_slice<'a, T: BedrockDecode<'a>>(slice: &'a [u8]) -> Result<T, Error> {
    let mut rest = slice;
    let value = T::decode(&mut rest)?;
    if !rest.is_empty() {
        return Err(Error::TrailingBytes);
    }
    Ok(value)
}
