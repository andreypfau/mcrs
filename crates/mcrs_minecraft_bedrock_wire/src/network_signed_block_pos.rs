use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::VarI32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NetworkSignedBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BedrockEncode for NetworkSignedBlockPos {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        VarI32(self.x).encode(w)?;
        VarI32(self.y).encode(w)?;
        VarI32(self.z).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for NetworkSignedBlockPos {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(x) = VarI32::decode(r)?;
        let VarI32(y) = VarI32::decode(r)?;
        let VarI32(z) = VarI32::decode(r)?;
        Ok(Self { x, y, z })
    }
}
