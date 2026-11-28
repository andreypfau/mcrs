use core::ops::RangeInclusive;
use mcrs_voxel::{BlockPos, SectionPos};

#[derive(Clone, Debug)]
pub struct SearchRegion {
    pub(crate) min_y: i32,
    pub(crate) max_y: i32,
    chunks_x: RangeInclusive<i32>,
    chunks_z: RangeInclusive<i32>,
}
impl SearchRegion {
    #[must_use]
    pub fn new(mob: BlockPos, length: f32, build_height: RangeInclusive<i32>) -> Self {
        let radius = (length + 8.0) as i32;
        Self {
            min_y: *build_height.start(),
            max_y: *build_height.end(),
            chunks_x: (mob.x.saturating_sub(radius) >> 4)..=(mob.x.saturating_add(radius) >> 4),
            chunks_z: (mob.z.saturating_sub(radius) >> 4)..=(mob.z.saturating_add(radius) >> 4),
        }
    }
    #[must_use]
    pub(crate) fn contains_section(&self, at: SectionPos) -> bool {
        self.chunks_x.contains(&at.x)
            && self.chunks_z.contains(&at.z)
            && at.y >= self.min_y >> 4
            && at.y <= self.max_y >> 4
    }
    #[must_use]
    pub fn contains(&self, at: BlockPos) -> bool {
        (self.min_y..=self.max_y).contains(&at.y) && self.contains_section(SectionPos::from(at))
    }
}
