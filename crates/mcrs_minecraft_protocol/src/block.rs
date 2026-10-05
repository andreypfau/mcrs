use crate::registry::{decode_registry_id, encode_registry_id};
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_registry::BlockStateId;
use std::io::Write;

impl From<VoxelId> for VarInt {
    fn from(id: VoxelId) -> Self {
        VarInt(i32::from(id.0))
    }
}

impl From<BlockStateId> for VarInt {
    fn from(id: BlockStateId) -> Self {
        VarInt(i32::from(id.0))
    }
}

impl Encode for BlockStateId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.0, w)
    }
}

impl Decode<'_> for BlockStateId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_registry_id(r).map(BlockStateId)
    }
}
