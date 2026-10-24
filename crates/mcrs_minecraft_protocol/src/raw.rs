use std::fmt;
use std::io::Write;
use std::marker::PhantomData;

use crate::{ConnectionState, Encode, Packet, PacketSide, Result};

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
    fn encode(&self, mut w: impl Write) -> Result<()> {
        Ok(w.write_all(self.0)?)
    }
}
