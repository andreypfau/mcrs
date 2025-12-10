use std::io::Write;
use anyhow::Context;
use derive_more::{Deref, From, Into};
use crate::{Decode, Encode, VarInt};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug, From, Into, Deref)]
pub struct BlockStateId(pub u16);

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

pub struct BlockId(pub u16);

impl Encode for BlockId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0 as i32).encode(w)
    }
}

impl Decode<'_> for BlockId {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let errmsg = "invalid block kind ID";
        Ok(BlockId(id.try_into().context(errmsg)?))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug, From, Into)]
pub struct BlockEntityId(pub u16);

impl Encode for BlockEntityId {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0 as i32).encode(w)
    }
}

impl<'a> Decode<'a> for BlockEntityId {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?;
        Ok(Self(id.0.try_into().with_context(|| format!("id {}", id.0))?))
    }
}