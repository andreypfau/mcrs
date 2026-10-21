use std::fmt;
use std::io::Write;
use std::marker::PhantomData;
use std::mem;

use anyhow::ensure;
use derive_more::{Deref, DerefMut, From, Into};

use crate::{Bounded, ConnectionState, Decode, Encode, Packet, PacketSide};

/// While [encoding], the contained slice is written directly to the output
/// without any length prefix or metadata.
///
/// While [decoding], the remainder of the input is returned as the contained
/// slice. The input will be at the EOF state after this is decoded.
///
/// [encoding]: Encode
/// [decoding]: Decode
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Deref, DerefMut, From, Into,
)]
pub struct RawBytes<'a>(pub &'a [u8]);

impl Encode for RawBytes<'_> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(self.0)?)
    }
}

impl<'a> Decode<'a> for RawBytes<'a> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self(mem::take(r)))
    }
}

/// Raises an encoding error if the inner slice is longer than `MAX_BYTES`.
impl<const MAX_BYTES: usize> Encode for Bounded<RawBytes<'_>, MAX_BYTES> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        ensure!(
            self.len() <= MAX_BYTES,
            "cannot encode more than {MAX_BYTES} raw bytes (got {} bytes)",
            self.len()
        );

        self.0.encode(w)
    }
}

/// Raises a decoding error if the remainder of the input is larger than
/// `MAX_BYTES`.
impl<'a, const MAX_BYTES: usize> Decode<'a> for Bounded<RawBytes<'a>, MAX_BYTES> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        ensure!(
            r.len() <= MAX_BYTES,
            "remainder of input exceeds max of {MAX_BYTES} bytes (got {} bytes)",
            r.len()
        );

        Ok(Bounded(RawBytes::decode(r)?))
    }
}

/// The body of a packet `P`, encoded ahead of time, written under the id of `P`.
pub struct EncodedBody<'a, P>(&'a [u8], PhantomData<fn() -> P>);

impl<'a, P: Packet> EncodedBody<'a, P> {
    pub fn new(body: &'a [u8]) -> Self {
        Self(body, PhantomData)
    }
}

impl<P: Packet> fmt::Debug for EncodedBody<'_, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} bytes)", P::NAME, self.0.len())
    }
}

impl<P: Packet> Packet for EncodedBody<'_, P> {
    const ID: i32 = P::ID;
    const NAME: &'static str = P::NAME;
    const SIDE: PacketSide = P::SIDE;
    const STATE: ConnectionState = P::STATE;
}

impl<P> Encode for EncodedBody<'_, P> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(self.0)?)
    }
}
