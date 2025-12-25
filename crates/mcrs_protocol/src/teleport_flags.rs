use crate::{Decode, Encode};
use std::io::Write;

/// For the first 8 values set means relative value while unset means absolute
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum PositionFlag {
    X,
    Y,
    Z,
    YRot,
    XRot,
    DeltaX,
    DeltaY,
    DeltaZ,
    RotateDelta,
}

impl PositionFlag {
    fn get_mask(&self) -> i32 {
        match self {
            PositionFlag::X => 1 << 0,
            PositionFlag::Y => 1 << 1,
            PositionFlag::Z => 1 << 2,
            PositionFlag::YRot => 1 << 3,
            PositionFlag::XRot => 1 << 4,
            PositionFlag::DeltaX => 1 << 5,
            PositionFlag::DeltaY => 1 << 6,
            PositionFlag::DeltaZ => 1 << 7,
            PositionFlag::RotateDelta => 1 << 8,
        }
    }

    pub fn get_bitfield(flags: &[PositionFlag]) -> i32 {
        flags.iter().fold(0, |acc, flag| acc | flag.get_mask())
    }
}

impl Encode for Vec<PositionFlag> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        let bitfield = PositionFlag::get_bitfield(self.as_slice());
        bitfield.encode(w)
    }
}

impl<'a> Decode<'a> for Vec<PositionFlag> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let bitfield = i32::decode(r)?;
        let mut flags = Vec::with_capacity(9);
        for i in 0..9 {
            if (bitfield & (1 << i)) != 0 {
                let flag = match i {
                    0 => PositionFlag::X,
                    1 => PositionFlag::Y,
                    2 => PositionFlag::Z,
                    3 => PositionFlag::YRot,
                    4 => PositionFlag::XRot,
                    5 => PositionFlag::DeltaX,
                    6 => PositionFlag::DeltaY,
                    7 => PositionFlag::DeltaZ,
                    8 => PositionFlag::RotateDelta,
                    _ => continue,
                };
                flags.push(flag);
            }
        }
        Ok(flags)
    }
}
