use std::io::Write;

use crate::{Decode, Encode};
use mcrs_minecraft_core::BlockPos;
pub use mcrs_minecraft_item::component::GlobalPos;

impl Encode for GlobalPos {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.dimension.encode(&mut w)?;
        self.pos.encode(w)
    }
}

impl Decode<'_> for GlobalPos {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(GlobalPos {
            dimension: Decode::decode(r)?,
            pos: BlockPos::decode(r)?,
        })
    }
}
