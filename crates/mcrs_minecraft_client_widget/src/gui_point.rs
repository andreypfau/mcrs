/// A point in GUI units, such as the mouse position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GuiPoint {
    pub x: i32,
    pub y: i32,
}

impl GuiPoint {
    /// The point (`x`, `y`).
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
