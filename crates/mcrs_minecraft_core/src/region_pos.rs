use crate::ColumnPos;

/// A region file: 32 by 32 columns.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RegionPos {
    pub x: i32,
    pub z: i32,
}

impl RegionPos {
    pub const BITS: usize = 5;
    pub const SIZE: usize = 1 << Self::BITS;
    pub const MASK: usize = Self::SIZE - 1;

    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub const fn column_at(self, local_x: i32, local_z: i32) -> ColumnPos {
        ColumnPos::new(
            (self.x << Self::BITS) + local_x,
            (self.z << Self::BITS) + local_z,
        )
    }
}

impl From<ColumnPos> for RegionPos {
    fn from(pos: ColumnPos) -> Self {
        Self::new(pos.x >> Self::BITS, pos.z >> Self::BITS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_columns_fall_in_the_region_below_zero() {
        let column = ColumnPos::new(-1, 33);
        let region = RegionPos::from(column);
        assert_eq!(region, RegionPos::new(-1, 1));
        assert_eq!((column.region_local_x(), column.region_local_z()), (31, 1));
        assert_eq!(region.column_at(31, 1), column);
    }
}
