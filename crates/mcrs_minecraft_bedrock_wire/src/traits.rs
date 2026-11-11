use alloc::vec::Vec;

use crate::error::Error;

/// A value with a Bedrock wire form.
pub trait BedrockEncode {
    /// Appends the wire form of `self` to `w`.
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error>;

    /// Appends the wire form of each item, without a count.
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

/// A value read from its Bedrock wire form, possibly borrowing from the input.
pub trait BedrockDecode<'a>: Sized {
    /// Reads a value from the front of `r` and advances `r` past it.
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error>;
}
