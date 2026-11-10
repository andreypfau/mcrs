//! Bedrock Edition wire encoding: zigzag varints, little-endian primitives, and no_std traits.
#![no_std]

extern crate alloc;

mod error;
mod primitives;
mod string;
mod traits;
mod var_int;

pub use error::Error;
pub use traits::{BedrockDecode, BedrockEncode};
pub use var_int::{VarI32, VarI64, VarU32, VarU64};

/// Serializes a value into a newly allocated `Vec<u8>`.
#[must_use = "the encoded bytes are the only output"]
pub fn to_bytes<T: BedrockEncode + ?Sized>(val: &T) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut buf = alloc::vec::Vec::new();
    val.encode(&mut buf)?;
    Ok(buf)
}

/// Deserializes a value from a byte slice.
#[must_use = "the decoded value is the only output"]
pub fn from_slice<'a, T: BedrockDecode<'a>>(slice: &'a [u8]) -> Result<T, Error> {
    let mut r = slice;
    let val = T::decode(&mut r)?;
    if !r.is_empty() {
        return Err(Error::TrailingBytes);
    }
    Ok(val)
}
