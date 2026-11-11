use alloc::vec::Vec;

use ::glam::{Vec2, Vec3};

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};

impl BedrockEncode for Vec2 {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.x.encode(w)?;
        self.y.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for Vec2 {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(Self::new(f32::decode(r)?, f32::decode(r)?))
    }
}

impl BedrockEncode for Vec3 {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.x.encode(w)?;
        self.y.encode(w)?;
        self.z.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for Vec3 {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(Self::new(f32::decode(r)?, f32::decode(r)?, f32::decode(r)?))
    }
}
