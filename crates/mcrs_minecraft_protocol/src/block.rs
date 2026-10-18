use crate::registry::{decode_registry_id, encode_registry_id};
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockStateId;
use std::io::Write;

impl From<VoxelId> for VarInt {
    fn from(id: VoxelId) -> Self {
        VarInt(i32::from(id.0))
    }
}

/// A block state as it travels on the wire. The side that holds the block definitions turns it
/// into a [`BlockStateId`] with `BlockStateId::checked(number, definitions.state_count())`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockStateNumber(pub u16);

impl From<BlockStateId> for BlockStateNumber {
    fn from(id: BlockStateId) -> Self {
        Self(id.number())
    }
}

impl Encode for BlockStateNumber {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.0, w)
    }
}

impl Decode<'_> for BlockStateNumber {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_registry_id(r).map(Self)
    }
}
