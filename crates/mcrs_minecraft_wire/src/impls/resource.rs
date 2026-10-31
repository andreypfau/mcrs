use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;

use mcrs_minecraft_resource::{Id, ResourceKey, ResourceLocation};

use crate::{Context, Decode, Encode, Error, Result, VarInt};

impl<S: AsRef<str>> Encode for ResourceLocation<S> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.as_str().encode(w)
    }
}

impl<'a> Decode<'a> for ResourceLocation<Cow<'a, str>> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        ResourceLocation::read_cow(<Cow<'a, str>>::decode(r)?).map_err(Error::other)
    }
}

impl Decode<'_> for ResourceLocation<Arc<str>> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        ResourceLocation::read(<&str>::decode(r)?).map_err(Error::other)
    }
}

impl<T, S: AsRef<str>> Encode for ResourceKey<T, S> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.location().encode(w)
    }
}

impl<T> Decode<'_> for ResourceKey<T, Arc<str>> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(ResourceKey::from_location(ResourceLocation::decode(r)?))
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
