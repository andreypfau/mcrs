#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorldHeight {
    pub min_y: i32,
    pub height: u32,
}

impl WorldHeight {
    #[must_use]
    pub fn is_outside(self, y: i32) -> bool {
        let top = i64::from(self.min_y) + i64::from(self.height) - 1;
        y <= self.min_y || i64::from(y) > top
    }
}
