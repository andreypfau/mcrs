use alloc::vec::Vec;
use core::ops::BitOr;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, NetworkBlockPos, VarU32};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x15)]
pub struct UpdateBlock {
    pub position: NetworkBlockPos,
    pub block_runtime_id: VarU32,
    pub flags: UpdateBlockFlags,
    pub layer: BlockLayer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UpdateBlockFlags(pub u32);

impl UpdateBlockFlags {
    pub const NEIGHBOURS: Self = Self(1);
    pub const NETWORK: Self = Self(1 << 1);
    pub const NO_GRAPHICS: Self = Self(1 << 2);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarU32)]
pub enum BlockLayer {
    Normal = 0,
    Liquid = 1,
}
