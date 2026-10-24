use std::io::Write;
use std::mem;
use std::ops::{Deref, DerefMut};

use crate::impls::check_len;
use crate::{Bounded, Decode, Encode, Result};

/// While [encoding], the contained slice is written directly to the output
/// without any length prefix or metadata.
///
/// While [decoding], the remainder of the input is returned as the contained
/// slice. The input will be at the EOF state after this is decoded.
///
/// [encoding]: Encode
/// [decoding]: Decode
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct RawBytes<'a>(pub &'a [u8]);

impl<'a> Deref for RawBytes<'a> {
    type Target = &'a [u8];

    fn deref(&self) -> &&'a [u8] {
        &self.0
    }
}

impl DerefMut for RawBytes<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> From<&'a [u8]> for RawBytes<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        RawBytes(bytes)
    }
}

impl<'a> From<RawBytes<'a>> for &'a [u8] {
    fn from(bytes: RawBytes<'a>) -> Self {
        bytes.0
    }
}

impl Encode for RawBytes<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        Ok(w.write_all(self.0)?)
    }
}

impl<'a> Decode<'a> for RawBytes<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Ok(Self(mem::take(r)))
    }
}

/// Raises an encoding error if the inner slice is longer than `MAX_BYTES`.
impl<const MAX_BYTES: usize> Encode for Bounded<RawBytes<'_>, MAX_BYTES> {
    fn encode(&self, w: impl Write) -> Result<()> {
        check_len(self.len(), MAX_BYTES)?;
        self.0.encode(w)
    }
}

/// Raises a decoding error if the remainder of the input is larger than
/// `MAX_BYTES`.
impl<'a, const MAX_BYTES: usize> Decode<'a> for Bounded<RawBytes<'a>, MAX_BYTES> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        check_len(r.len(), MAX_BYTES)?;
        Ok(Bounded(RawBytes::decode(r)?))
    }
}
