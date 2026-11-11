use alloc::vec::Vec;
use core::mem;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::VarU32;

const MAX_PREALLOC_BYTES: usize = 1 << 20;

impl<T: BedrockEncode> BedrockEncode for [T] {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let count = u32::try_from(self.len()).map_err(|_| Error::LengthOverflow)?;
        VarU32(count).encode(w)?;
        T::encode_slice(self, w)
    }
}

impl<'a, T: BedrockDecode<'a>> BedrockDecode<'a> for Vec<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarU32(count) = VarU32::decode(r)?;
        let count = usize::try_from(count).map_err(|_| Error::LengthOverflow)?;
        decode_items(count, r)
    }
}

impl<'a> BedrockDecode<'a> for &'a [u8] {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarU32(len) = VarU32::decode(r)?;
        let len = usize::try_from(len).map_err(|_| Error::LengthOverflow)?;
        let Some((bytes, rest)) = r.split_at_checked(len) else {
            return Err(Error::UnexpectedEof);
        };
        *r = rest;
        Ok(bytes)
    }
}

/// Reads `count` items of `T`, reserving no more than the input can hold.
pub fn decode_items<'a, T: BedrockDecode<'a>>(
    count: usize,
    r: &mut &'a [u8],
) -> Result<Vec<T>, Error> {
    let mut items = Vec::with_capacity(bounded_capacity::<T>(count, r));
    for _ in 0..count {
        items.push(T::decode(r)?);
    }
    Ok(items)
}

/// The capacity to reserve for `declared` items of `T` that the bytes of `remaining` hold.
///
/// A declared count is untrusted: each item takes at least one byte of input, so the input
/// length bounds the count, and a fixed byte budget bounds large items.
#[must_use]
pub(crate) fn bounded_capacity<T>(declared: usize, remaining: &[u8]) -> usize {
    declared
        .min(remaining.len())
        .min(MAX_PREALLOC_BYTES / mem::size_of::<T>().max(1))
}
