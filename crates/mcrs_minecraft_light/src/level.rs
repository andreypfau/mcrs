use mcrs_voxel_math::ColumnPos;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use std::ops::RangeInclusive;

pub const SECTION_WIDTH: i32 = BLOCKS::SIZE as i32;

/// A vertical block column, identified by its horizontal position.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockColumn {
    pub x: i32,
    pub z: i32,
}

impl BlockColumn {
    pub const fn section_column(self) -> ColumnPos {
        ColumnPos {
            x: self.x >> BLOCKS::BITS,
            z: self.z >> BLOCKS::BITS,
        }
    }
}

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
        (0..BLOCKS::VOLUME).map(LocalPos::from_index)
    }
}

/// A light value, always in `0..=15`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct LightLevel(u8);

impl LightLevel {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(15);

    /// Clamps into the valid range rather than panicking: light arithmetic
    /// saturates everywhere else too, and a panic here would be a denial of
    /// service triggerable by a badly configured block table.
    pub const fn new(value: u8) -> Self {
        Self(if value > 15 { 15 } else { value })
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn saturating_sub(self, amount: u8) -> Self {
        Self(self.0.saturating_sub(amount))
    }

    /// Combined brightness as gameplay and rendering see it.
    ///
    /// `sky_darken` is applied here and never stored, so that a weather change
    /// costs nothing. Callers that must be independent of time of day — crop
    /// growth, passive spawning — pass zero.
    pub fn brightness(block: Self, sky: Self, sky_darken: u8) -> Self {
        block.max(sky.saturating_sub(sky_darken))
    }
}

/// Vertical extent of the world, in sections.
///
/// Light is tracked one section beyond the world in each direction. The extra
/// sections give sky light somewhere to come from and block light somewhere to
/// die.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct LightBounds {
    pub min_section_y: i32,
    pub max_section_y: i32,
}

impl LightBounds {
    pub const fn new(min_section_y: i32, max_section_y: i32) -> Self {
        Self {
            min_section_y,
            max_section_y,
        }
    }

    pub const fn from_dimension(min_y: i32, section_count: u32) -> Self {
        let min_section_y = min_y >> BLOCKS::BITS;
        Self {
            min_section_y,
            max_section_y: min_section_y + section_count as i32 - 1,
        }
    }

    pub const fn min_light_section_y(self) -> i32 {
        self.min_section_y - 1
    }

    pub const fn max_light_section_y(self) -> i32 {
        self.max_section_y + 1
    }

    pub const fn light_sections(self) -> RangeInclusive<i32> {
        self.min_light_section_y()..=self.max_light_section_y()
    }

    /// Lowest block Y that belongs to the world itself.
    pub const fn min_block_y(self) -> i32 {
        self.min_section_y * SECTION_WIDTH
    }

    /// Highest block Y that belongs to the world itself.
    pub const fn max_block_y(self) -> i32 {
        self.max_section_y * SECTION_WIDTH + SECTION_WIDTH - 1
    }

    /// True for positions above or below the world, where there are no blocks
    /// at all — as opposed to positions inside it that merely are not loaded.
    pub const fn is_outside(self, y: i32) -> bool {
        y < self.min_block_y() || y > self.max_block_y()
    }

    /// Lowest block Y that can hold light.
    pub const fn min_light_y(self) -> i32 {
        self.min_light_section_y() * SECTION_WIDTH
    }

    /// Highest block Y that can hold light.
    pub const fn max_light_y(self) -> i32 {
        self.max_light_section_y() * SECTION_WIDTH + SECTION_WIDTH - 1
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
    }

    #[test]
    fn overworld_bounds_span_the_declared_sections() {
        let bounds = LightBounds::from_dimension(-64, 24);
        assert_eq!(bounds.min_section_y, -4);
        assert_eq!(bounds.max_section_y, 19);
        assert_eq!(bounds.min_block_y(), -64);
        assert_eq!(bounds.max_block_y(), 319);
        assert!(bounds.is_outside(-65));
        assert!(bounds.is_outside(320));
    }
}
