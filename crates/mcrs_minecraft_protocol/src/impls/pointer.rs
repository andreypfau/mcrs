use std::borrow::Cow;
use std::io::Write;

use crate::{Decode, Encode};

impl<T: Encode + ?Sized> Encode for &T {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        (**self).encode(w)
    }
}

impl<T: Encode + ?Sized> Encode for Box<T> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_ref().encode(w)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Box<T> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        T::decode(r).map(Box::new)
    }
}

impl<'a, B> Encode for Cow<'a, B>
where
    B: ToOwned + Encode + ?Sized,
{
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.as_ref().encode(w)
    }
}

impl<'a, 'b, B> Decode<'a> for Cow<'b, B>
where
    B: ToOwned + ?Sized,
    B::Owned: Decode<'a>,
{
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        B::Owned::decode(r).map(Cow::Owned)
    }
}
