use crate::{Decode, Encode, VarInt};
use anyhow::Context;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_registry::BlockStateId;
use std::io::Write;

impl From<VoxelId> for VarInt {
    fn from(id: VoxelId) -> Self {
        VarInt(id.0 as i32)
    }
}

impl From<BlockStateId> for VarInt {
    fn from(id: BlockStateId) -> Self {
        VarInt(id.0 as i32)
    }
}

impl Encode for BlockStateId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0 as i32).encode(w)
    }
}

impl Decode<'_> for BlockStateId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let errmsg = "invalid block state ID";

        Ok(BlockStateId(id.try_into().context(errmsg)?))
    }
}
