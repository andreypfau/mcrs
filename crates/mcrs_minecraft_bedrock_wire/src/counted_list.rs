use alloc::vec::Vec;

use crate::error::Error;
use crate::slice::decode_items;
use crate::traits::{BedrockDecode, BedrockEncode};

/// Appends `items` behind a count of the fixed-width little-endian type `C`, such as `u16` or `u32`.
pub fn encode_counted_list<C, T>(items: &[T], w: &mut Vec<u8>) -> Result<(), Error>
where
    C: BedrockEncode + TryFrom<usize>,
    T: BedrockEncode,
{
    let count = C::try_from(items.len()).map_err(|_| Error::LengthOverflow)?;
    count.encode(w)?;
    T::encode_slice(items, w)
}

/// Reads a list written by [`encode_counted_list`] with the same count type `C`.
pub fn decode_counted_list<'a, C, T>(r: &mut &'a [u8]) -> Result<Vec<T>, Error>
where
    C: BedrockDecode<'a>,
    usize: TryFrom<C>,
    T: BedrockDecode<'a>,
{
    let count = usize::try_from(C::decode(r)?).map_err(|_| Error::LengthOverflow)?;
    decode_items(count, r)
}
