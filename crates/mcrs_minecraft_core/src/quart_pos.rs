use crate::{BlockPos, ColumnPos, SectionPos};

/// A biome cell: 4 by 4 by 4 blocks.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct QuartPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl QuartPos {
    pub const BITS: usize = 2;
    pub const SIZE: usize = 1 << Self::BITS;
    pub const MASK: usize = Self::SIZE - 1;
    const PER_SECTION_BITS: usize = SectionPos::BITS - Self::BITS;

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub const fn of(pos: BlockPos) -> Self {
        let pos = pos.as_ivec3();
        Self::new(
            pos.x >> Self::BITS,
            pos.y >> Self::BITS,
            pos.z >> Self::BITS,
        )
    }

    pub const fn column(self) -> ColumnPos {
        ColumnPos::new(
            self.x >> Self::PER_SECTION_BITS,
            self.z >> Self::PER_SECTION_BITS,
        )
    }

    pub const fn min_block_y(self) -> i32 {
        self.y << Self::BITS
    }

    /// The cell's index inside its section's 4 by 4 by 4 biome grid, as x, y, z.
    pub const fn section_local(self) -> [usize; 3] {
        let mask = (1 << Self::PER_SECTION_BITS) - 1;
        [
            (self.x & mask) as usize,
            (self.y & mask) as usize,
            (self.z & mask) as usize,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negative_block_rounds_down_to_its_cell_and_section() {
        let quart = QuartPos::of(BlockPos::new(-1, 17, -16));
        assert_eq!(quart, QuartPos::new(-1, 4, -4));
        assert_eq!(quart.column(), ColumnPos::new(-1, -1));
        assert_eq!(quart.section_local(), [3, 0, 0]);
        assert_eq!(quart.min_block_y(), 16);
    }
}
