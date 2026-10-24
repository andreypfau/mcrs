use std::borrow::Cow;
use std::io::Write;
use std::mem::{self, MaybeUninit};
use std::slice;

use super::{cautious_capacity, check_len, decode_len, encode_len};
use crate::{Bounded, Decode, Encode, ErrorKind, RawBytes, Result};

/// Like tuples, fixed-length arrays are encoded and decoded without a VarInt
/// length prefix.
impl<T: Encode, const N: usize> Encode for [T; N] {
    fn encode(&self, w: impl Write) -> Result<()> {
        T::encode_slice(self, w)
    }
}

impl<'a, T: Decode<'a>, const N: usize> Decode<'a> for [T; N] {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        // TODO: rewrite using std::array::try_from_fn when stabilized?

        let mut data: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        for (i, elem) in data.iter_mut().enumerate() {
            match T::decode(r) {
                Ok(val) => {
                    elem.write(val);
                }
                Err(e) => {
                    // Call destructors for values decoded so far.
                    for elem in &mut data[..i] {
                        unsafe { elem.assume_init_drop() };
                    }
                    return Err(e);
                }
            }
        }

        // All values in `data` are initialized.
        unsafe { Ok(mem::transmute_copy(&data)) }
    }
}

/// References to fixed-length arrays are not length prefixed.
impl<'a, const N: usize> Decode<'a> for &'a [u8; N] {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let (arr, remaining) = r.split_first_chunk::<N>().ok_or(ErrorKind::UnexpectedEnd)?;
        *r = remaining;
        Ok(arr)
    }
}

impl<T: Encode> Encode for [T] {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        encode_len(self.len(), &mut w)?;
        T::encode_slice(self, w)
    }
}

impl<T: Encode, const MAX_LEN: usize> Encode for Bounded<&'_ [T], MAX_LEN> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        check_len(self.len(), MAX_LEN)?;
        encode_len(self.len(), &mut w)?;
        T::encode_slice(self, w)
    }
}

impl<'a> Decode<'a> for &'a [u8] {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let len = decode_len(r)?;
        let (res, remaining) = r.split_at_checked(len).ok_or(ErrorKind::UnexpectedEnd)?;
        *r = remaining;
        Ok(res)
    }
}

impl<'a, const MAX_LEN: usize> Decode<'a> for Bounded<&'a [u8], MAX_LEN> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let res = <&[u8]>::decode(r)?;
        check_len(res.len(), MAX_LEN)?;
        Ok(Bounded(res))
    }
}

impl<'a> Decode<'a> for &'a [i8] {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let bytes = <&[u8]>::decode(r)?;

        // SAFETY: i8 and u8 have the same layout.
        let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<i8>(), bytes.len()) };

        Ok(bytes)
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_slice().encode(w)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Vec<T> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let len = decode_len(r)?;

        let mut vec = Vec::with_capacity(cautious_capacity::<T>(len, r));

        for _ in 0..len {
            vec.push(T::decode(r)?);
        }

        Ok(vec)
    }
}

impl<T: Encode, const MAX_LEN: usize> Encode for Bounded<Vec<T>, MAX_LEN> {
    fn encode(&self, w: impl Write) -> Result<()> {
        Bounded::<_, MAX_LEN>(self.0.as_slice()).encode(w)
    }
}

impl<'a, T: Decode<'a>, const MAX_LEN: usize> Decode<'a> for Bounded<Vec<T>, MAX_LEN> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let len = decode_len(r)?;
        check_len(len, MAX_LEN)?;

        let mut vec = Vec::with_capacity(cautious_capacity::<T>(len, r));

        for _ in 0..len {
            vec.push(T::decode(r)?);
        }

        Ok(Bounded(vec))
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Box<[T]> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Ok(Vec::decode(r)?.into_boxed_slice())
    }
}

impl<'a, T: Decode<'a>, const MAX_LEN: usize> Decode<'a> for Bounded<Box<[T]>, MAX_LEN> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Ok(Bounded::<Vec<_>, MAX_LEN>::decode(r)?.map_into())
    }
}

impl<'a, const MAX_LEN: usize> Decode<'a> for Bounded<Cow<'a, RawBytes<'a>>, MAX_LEN> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let Bounded(raw) = Bounded::<RawBytes<'a>, MAX_LEN>::decode(r)?;
        Ok(Bounded(Cow::Owned(raw)))
    }
}
