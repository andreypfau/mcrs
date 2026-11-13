use crate::alignment::Alignment;
use crate::arithmetic;
use crate::gui_extent::GuiExtent;
use crate::gui_point::GuiPoint;

/// A rectangle in GUI units: `x` and `y` are inside it, `x + width` and `y + height` are not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GuiRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl GuiRect {
    /// The rectangle at (`x`, `y`) of the given width and height.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The rectangle of `size` aligned in this one, with both offsets truncated toward zero.
    #[must_use]
    pub fn align(self, size: GuiExtent, alignment: Alignment) -> Self {
        let free_width = self.width.saturating_sub(size.width);
        let free_height = self.height.saturating_sub(size.height);
        Self::new(
            self.x
                .saturating_add(arithmetic::lerp(alignment.x, 0, free_width) as i32),
            self.y
                .saturating_add(arithmetic::lerp(alignment.y, 0, free_height) as i32),
            size.width,
            size.height,
        )
    }

    pub(crate) fn contains_f64(self, x: f64, y: f64) -> bool {
        let left = f64::from(self.x);
        let top = f64::from(self.y);
        left <= x
            && x < left + f64::from(self.width)
            && top <= y
            && y < top + f64::from(self.height)
    }

    pub(crate) fn contains_point(self, point: GuiPoint) -> bool {
        let (x, y) = (i64::from(point.x), i64::from(point.y));
        let (left, top) = (i64::from(self.x), i64::from(self.y));
        left <= x
            && x < left + i64::from(self.width)
            && top <= y
            && y < top + i64::from(self.height)
    }

    pub(crate) fn shifted_up(self, shift: i32) -> Self {
        Self {
            y: self.y.saturating_sub(shift),
            ..self
        }
    }

    pub(crate) fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }

    pub(crate) fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }
}
