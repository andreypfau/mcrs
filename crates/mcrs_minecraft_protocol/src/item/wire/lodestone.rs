use std::io::Write;

use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};

use crate::item::component::lodestone::*;
use crate::item::ctx::ctx_free;
use crate::{Decode, Encode};

impl Encode for GlobalPosValue {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.dimension.location().encode(&mut w)?;
        self.pos.encode(w)
    }
}

impl Decode<'_> for GlobalPosValue {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GlobalPosValue {
            dimension: ResourceKey::from_location(ResourceLocation::decode(r)?),
            pos: BlockPos::decode(r)?,
        })
    }
}

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
