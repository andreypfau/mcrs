use std::io::Write;
use std::slice;

use crate::{Decode, Encode, ErrorKind, Result};

pub(crate) fn take<const N: usize>(r: &mut &[u8]) -> Result<[u8; N]> {
    let (bytes, rest) = r.split_first_chunk::<N>().ok_or(ErrorKind::UnexpectedEnd)?;
    *r = rest;
    Ok(*bytes)
}

impl Encode for bool {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        Ok(w.write_all(&[u8::from(*self)])?)
    }

    fn encode_slice(slice: &[bool], mut w: impl Write) -> Result<()> {
        // SAFETY: Bools have the same layout as u8.
        let bytes = unsafe { slice::from_raw_parts(slice.as_ptr().cast::<u8>(), slice.len()) };
        // Bools are guaranteed to have the correct bit pattern.
        Ok(w.write_all(bytes)?)
    }
}

impl Decode<'_> for bool {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(u8::decode(r)? != 0)
    }
}

impl Encode for u8 {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        Ok(w.write_all(&[*self])?)
    }

    fn encode_slice(slice: &[u8], mut w: impl Write) -> Result<()> {
        Ok(w.write_all(slice)?)
    }
}

impl Encode for i8 {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        Ok(w.write_all(&self.to_be_bytes())?)
    }

    fn encode_slice(slice: &[i8], mut w: impl Write) -> Result<()> {
        // SAFETY: i8 has the same layout as u8.
        let bytes = unsafe { slice::from_raw_parts(slice.as_ptr().cast::<u8>(), slice.len()) };
        Ok(w.write_all(bytes)?)
    }
}

macro_rules! big_endian {
    ($($ty:ty),* $(,)?) => {$(
        impl Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> Result<Self> {
                take(r).map(<$ty>::from_be_bytes)
            }
        }
    )*};
}

macro_rules! big_endian_encode {
    ($($ty:ty),* $(,)?) => {$(
        impl Encode for $ty {
            fn encode(&self, mut w: impl Write) -> Result<()> {
                Ok(w.write_all(&self.to_be_bytes())?)
            }
        }
    )*};
}

big_endian!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);
big_endian_encode!(u16, i16, u32, i32, u64, i64, u128, i128, f32, f64);
