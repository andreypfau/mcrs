/// The minimum `y` and the height of a dimension (FBK-TCK-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldHeight {
    /// The `y` of the lowest block layer.
    pub min_y: i32,
    /// The number of block layers.
    pub height: u32,
}

impl WorldHeight {
    /// True when the cell at `y` is outside: the lowest layer `min_y` counts as outside, the
    /// highest layer `min_y + height - 1` as inside.
    #[must_use]
    pub fn is_outside(self, y: i32) -> bool {
        let top = i64::from(self.min_y) + i64::from(self.height) - 1;
        y <= self.min_y || i64::from(y) > top
    }
}
