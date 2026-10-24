//! Implementations of [`Encode`](crate::Encode) and [`Decode`](crate::Decode)
//! on foreign types.

#[cfg(feature = "glam")]
mod glam;
mod map;
mod option;
mod pointer;
pub(crate) mod primitive;
pub(crate) use primitive::take;
mod sequence;
mod string;
mod tuple;
#[cfg(feature = "uuid")]
mod uuid;

/// Prevents preallocating too much memory in case we get a malicious or invalid
/// sequence length.
const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

fn cautious_capacity<T>(declared: usize) -> usize {
    declared.min(MAX_PREALLOC_BYTES / size_of::<T>().max(1))
}

fn decode_len(r: &mut &[u8]) -> crate::Result<usize> {
    let crate::VarInt(len) = crate::Decode::decode(r)?;
    usize::try_from(len).map_err(|_| crate::ErrorKind::NegativeLength(len).into())
}

fn encode_len(len: usize, w: impl std::io::Write) -> crate::Result<()> {
    let max = i32::MAX as usize;
    let len = i32::try_from(len).map_err(|_| crate::ErrorKind::TooLong { len, max })?;
    crate::Encode::encode(&crate::VarInt(len), w)
}

pub(crate) fn check_len(len: usize, max: usize) -> crate::Result<()> {
    if len > max {
        return Err(crate::ErrorKind::TooLong { len, max }.into());
    }
    Ok(())
}
