use crate::axis::Axis;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GuiExtent {
    pub width: i32,
    pub height: i32,
}

impl GuiExtent {
    #[must_use]
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }

    pub(crate) fn along(self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }
}
