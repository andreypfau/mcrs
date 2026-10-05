use std::io::Write;

use crate::item::component::lodestone::*;
use crate::item::ctx::ctx_free;
use crate::{Decode, Encode};

impl Encode for LodestoneTracker {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.target.encode(&mut w)?;
        self.tracked.encode(w)
    }
}

impl Decode<'_> for LodestoneTracker {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(LodestoneTracker {
            target: Option::decode(r)?,
            tracked: bool::decode(r)?,
        })
    }
}

ctx_free!(LodestoneTracker);
