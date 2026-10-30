use crate::{ColumnPos, Decode, Encode, Result};
use bitfield_struct::bitfield;
use derive_more::Deref;
use glam::DVec3;

#[derive(Clone, Copy, PartialEq, Debug, Default, Deref)]
pub struct Position(DVec3);

impl Position {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }
}

impl From<DVec3> for Position {
    fn from(value: DVec3) -> Self {
        Position(value)
    }
}

impl From<Position> for DVec3 {
    fn from(value: Position) -> Self {
        value.0
    }
}

impl Encode for Position {
    fn encode(&self, mut w: impl std::io::Write) -> Result<()> {
        let pos = self.0;
        pos.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for Position {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let pos = DVec3::decode(r)?;
        Ok(Position(pos))
    }
}

#[bitfield(u8)]
pub struct MoveFlags {
    pub on_ground: bool,
    pub horizontal_collision: bool,
    #[bits(6)]
    _pad: u8,
}

impl Encode for MoveFlags {
    fn encode(&self, mut w: impl std::io::Write) -> Result<()> {
        self.0.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for MoveFlags {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let byte = u8::decode(r)?;
        Ok(MoveFlags::from_bits(byte))
    }
}

impl From<Position> for ColumnPos {
    #[inline]
    fn from(pos: Position) -> Self {
        Self::from(*pos)
    }
}
