use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::VarI32;

/// A chunk position: a varint32 x, then a varint32 z.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NetworkChunkPos {
    /// The chunk x coordinate.
    pub x: i32,
    /// The chunk z coordinate.
    pub z: i32,
}

impl BedrockEncode for NetworkChunkPos {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        VarI32(self.x).encode(w)?;
        VarI32(self.z).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for NetworkChunkPos {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(x) = VarI32::decode(r)?;
        let VarI32(z) = VarI32::decode(r)?;
        Ok(Self { x, z })
    }
}
