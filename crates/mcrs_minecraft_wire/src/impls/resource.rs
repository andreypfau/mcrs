use std::io::Write;

use mcrs_minecraft_resource::{
    Id, ResourceKey, ResourceKeyBuf, ResourceLocation, ResourceLocationBuf,
};

use crate::impls::{check_len, encode_len};
use crate::{Context, Decode, Encode, Error, Result, VarInt};

/// The longest string the protocol writes; a location is ASCII, so its bytes are its chars.
const MAX_CHARS: usize = 32767;

/// Writes the canonical `namespace:path` text from the two parts, without a buffer.
impl Encode for ResourceLocation<'_> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        check_len(self.len(), MAX_CHARS)?;
        encode_len(self.len(), &mut w)?;
        w.write_all(self.namespace().as_bytes())?;
        w.write_all(b":")?;
        Ok(w.write_all(self.path().as_bytes())?)
    }
}

/// Borrows the parts from the frame.
impl<'a> Decode<'a> for ResourceLocation<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        ResourceLocation::read(<&'a str>::decode(r)?).map_err(Error::other)
    }
}

impl Encode for ResourceLocationBuf {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_str().encode(w)
    }
}

impl Decode<'_> for ResourceLocationBuf {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        ResourceLocationBuf::read(<&str>::decode(r)?).map_err(Error::other)
    }
}

impl<T> Encode for ResourceKey<'_, T> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.location().encode(w)
    }
}

impl<T> Encode for ResourceKeyBuf<T> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.location().encode(w)
    }
}

impl<T> Decode<'_> for ResourceKeyBuf<T> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(ResourceKeyBuf::from_location(ResourceLocationBuf::decode(
            r,
        )?))
    }
}

/// An id writes as its sixteen-bit number. Whether a registry holds that number is checked where
/// the id is looked up, since only the registries the connection holds know their size.
impl<R> Encode for Id<R> {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(i32::from(self.number())).encode(w)
    }
}

impl<R> Decode<'_> for Id<R> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let VarInt(raw) = VarInt::decode(r)?;
        u16::try_from(raw)
            .ok()
            .and_then(|number| Id::checked(number, usize::from(u16::MAX) + 1))
            .with_context(|| format!("registry id {raw} is outside 0..=65535"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_forms_write_the_bytes_of_the_canonical_text() {
        for text in ["stone", "minecraft:stone", "mcrs:block/alt"] {
            let borrowed = ResourceLocation::read(text).unwrap();
            let owned = ResourceLocationBuf::read(text).unwrap();
            let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
            borrowed.encode(&mut a).unwrap();
            owned.encode(&mut b).unwrap();
            owned.as_str().encode(&mut c).unwrap();
            assert_eq!((&a, &b), (&c, &c), "{text}");
            let mut frame = a.as_slice();
            assert_eq!(ResourceLocation::decode(&mut frame).unwrap(), owned);
            assert!(frame.is_empty());
        }
    }
}
