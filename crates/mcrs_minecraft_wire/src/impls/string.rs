use std::io::Write;

use super::{check_len, decode_len, encode_len};
use crate::{Bounded, Decode, Encode, ErrorKind, Result};

const DEFAULT_MAX_STRING_CHARS: usize = 32767;

impl Encode for str {
    fn encode(&self, w: impl Write) -> Result<()> {
        Bounded::<_, DEFAULT_MAX_STRING_CHARS>(self).encode(w)
    }
}

impl<const MAX_CHARS: usize> Encode for Bounded<&'_ str, MAX_CHARS> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        check_len(self.encode_utf16().count(), MAX_CHARS)?;
        encode_len(self.len(), &mut w)?;
        Ok(w.write_all(self.as_bytes())?)
    }
}

impl<'a> Decode<'a> for &'a str {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Ok(Bounded::<_, DEFAULT_MAX_STRING_CHARS>::decode(r)?.0)
    }
}

impl<'a, const MAX_CHARS: usize> Decode<'a> for Bounded<&'a str, MAX_CHARS> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let len = decode_len(r)?;
        let (res, remaining) = r.split_at_checked(len).ok_or(ErrorKind::UnexpectedEnd)?;
        let res = std::str::from_utf8(res)?;
        check_len(res.encode_utf16().count(), MAX_CHARS)?;

        *r = remaining;

        Ok(Bounded(res))
    }
}

impl Encode for String {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_str().encode(w)
    }
}

impl<const MAX_CHARS: usize> Encode for Bounded<String, MAX_CHARS> {
    fn encode(&self, w: impl Write) -> Result<()> {
        Bounded::<_, MAX_CHARS>(self.as_str()).encode(w)
    }
}

impl Decode<'_> for String {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(<&str>::decode(r)?.into())
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for Bounded<String, MAX_CHARS> {
    fn decode(r: &mut &'_ [u8]) -> Result<Self> {
        Ok(Bounded(Bounded::<&str, MAX_CHARS>::decode(r)?.0.into()))
    }
}

impl Decode<'_> for Box<str> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(<&str>::decode(r)?.into())
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for Bounded<Box<str>, MAX_CHARS> {
    fn decode(r: &mut &'_ [u8]) -> Result<Self> {
        Ok(Bounded(Bounded::<&str, MAX_CHARS>::decode(r)?.0.into()))
    }
}
