/// Where a vertical anchor sits, given the dimension's own extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightContext {
    pub min_y: i32,
    pub depth: i32,
    pub sea_level: i32,
}

impl HeightContext {
    #[inline]
    pub fn contains(self, y: i32) -> bool {
        y >= self.min_y && y < self.min_y + self.depth
    }
}
