use mcrs_voxel::ColumnPos;
use mcrs_voxel::SectionPos;
use std::ops::RangeInclusive;

pub const SECTION_WIDTH: i32 = SectionPos::SIZE as i32;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockColumn {
    pub x: i32,
    pub z: i32,
}

impl BlockColumn {
    pub const fn section_column(self) -> ColumnPos {
        ColumnPos {
            x: self.x >> SectionPos::BITS,
            z: self.z >> SectionPos::BITS,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct LightLevel(u8);

impl LightLevel {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(15);

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

    pub fn brightness(block: Self, sky: Self, sky_darken: u8) -> Self {
        block.max(sky.saturating_sub(sky_darken))
    }
}

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
        let min_section_y = min_y >> SectionPos::BITS;
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

    pub const fn min_block_y(self) -> i32 {
        self.min_section_y * SECTION_WIDTH
    }

    pub const fn max_block_y(self) -> i32 {
        self.max_section_y * SECTION_WIDTH + SECTION_WIDTH - 1
    }

    pub const fn is_outside(self, y: i32) -> bool {
        y < self.min_block_y() || y > self.max_block_y()
    }

    pub fn sky_flip_span(self, old: i32, new: i32) -> Option<(i32, i32)> {
        if old == new {
            return None;
        }
        let low = old.min(new).max(self.min_light_y());
        let high = old.max(new).saturating_sub(1).min(self.max_light_y());
        (low <= high).then_some((low, high))
    }

    pub const fn min_light_y(self) -> i32 {
        self.min_light_section_y() * SECTION_WIDTH
    }

    pub const fn max_light_y(self) -> i32 {
        self.max_light_section_y() * SECTION_WIDTH + SECTION_WIDTH - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
