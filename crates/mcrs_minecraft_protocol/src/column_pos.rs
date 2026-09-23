use std::io::Write;

use crate::{Decode, Encode, Position};

pub use mcrs_minecraft_core::ColumnPos;

impl Encode for ColumnPos {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.x.encode(&mut w)?;
        self.z.encode(&mut w)?;
        Ok(())
    }
}

impl Decode<'_> for ColumnPos {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let x = i32::decode(r)?;
        let z = i32::decode(r)?;
        Ok(ColumnPos { x, z })
    }
}

impl From<Position> for ColumnPos {
    #[inline]
    fn from(pos: Position) -> Self {
        Self::from(*pos)
    }
}
