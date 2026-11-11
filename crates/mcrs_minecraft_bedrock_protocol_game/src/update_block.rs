use alloc::vec::Vec;
use core::ops::BitOr;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, NetworkBlockPos, VarU32};

/// Sets one block of the client to a block state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x15)]
pub struct UpdateBlock {
    /// The position of the block.
    pub position: NetworkBlockPos,
    /// The block runtime id of the new block state.
    pub block_runtime_id: VarU32,
    /// The update flags; a server of mcrs writes neighbours and network.
    pub flags: UpdateBlockFlags,
    /// The layer of the block.
    pub layer: BlockLayer,
}

/// The flag set of [`UpdateBlock`], as a varuint32. A receiver accepts each value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UpdateBlockFlags(pub u32);

impl UpdateBlockFlags {
    /// Bit 0: neighbours.
    pub const NEIGHBOURS: Self = Self(1);
    /// Bit 1: network.
    pub const NETWORK: Self = Self(1 << 1);
    /// Bit 2: no graphics.
    pub const NO_GRAPHICS: Self = Self(1 << 2);
    /// Bit 3: priority.
    pub const PRIORITY: Self = Self(1 << 3);
}

impl BitOr for UpdateBlockFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BedrockEncode for UpdateBlockFlags {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        VarU32(self.0).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for UpdateBlockFlags {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarU32(flags) = VarU32::decode(r)?;
        Ok(Self(flags))
    }
}

/// The layer of a block position that [`UpdateBlock`] sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum BlockLayer {
    /// The normal layer.
    Normal = 0,
    /// The liquid layer.
    Liquid = 1,
}
