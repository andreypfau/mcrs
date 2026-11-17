use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::{VarI32, VarU32};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NetworkBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BedrockEncode for NetworkBlockPos {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        VarI32(self.x).encode(w)?;
        VarU32(self.y.cast_unsigned()).encode(w)?;
        VarI32(self.z).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for NetworkBlockPos {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(x) = VarI32::decode(r)?;
        let VarU32(y) = VarU32::decode(r)?;
        let VarI32(z) = VarI32::decode(r)?;
        Ok(Self {
            x,
            y: y.cast_signed(),
            z,
        })
    }
}
