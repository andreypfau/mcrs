use crate::{BlockPos, SectionPos};

/// Index of a block inside its own section: `x | z << 4 | y << 8`, twelve bits.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct LocalPos(u16);

impl LocalPos {
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        Self((x as u16 & 15) | ((z as u16 & 15) << 4) | ((y as u16 & 15) << 8))
    }

    pub const fn from_index(index: usize) -> Self {
        Self((index & 0xFFF) as u16)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn x(self) -> u8 {
        (self.0 & 15) as u8
    }

    pub const fn z(self) -> u8 {
        ((self.0 >> 4) & 15) as u8
    }

    pub const fn y(self) -> u8 {
        ((self.0 >> 8) & 15) as u8
    }

    pub fn all() -> impl Iterator<Item = LocalPos> {
        (0..SectionPos::VOLUME).map(LocalPos::from_index)
    }
}

impl From<BlockPos> for LocalPos {
    fn from(pos: BlockPos) -> Self {
        Self::new(pos.x as u8, pos.y as u8, pos.z as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_pos_packs_x_then_z_then_y() {
        let pos = LocalPos::new(3, 7, 11);
        assert_eq!(pos.index(), 3 | (11 << 4) | (7 << 8));
        assert_eq!((pos.x(), pos.y(), pos.z()), (3, 7, 11));
        assert_eq!(LocalPos::from(BlockPos::new(-13, 23, -5)), pos);
    }
}
