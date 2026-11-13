use crate::axis::Axis;
use crate::child::Child;
use crate::equal_gaps::{self, EqualGaps};
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;

/// Children in a row or a column, in order, a fixed gap apart: a grid of one row or one column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stack<'a> {
    axis: Axis,
    gap: i32,
    children: &'a [Child],
}

impl<'a> Stack<'a> {
    /// A row: the children from left to right, `gap` apart.
    #[must_use]
    pub const fn row(gap: i32, children: &'a [Child]) -> Self {
        Self {
            axis: Axis::Horizontal,
            gap,
            children,
        }
    }

    /// A column: the children from top to bottom, `gap` apart.
    #[must_use]
    pub const fn column(gap: i32, children: &'a [Child]) -> Self {
        Self {
            axis: Axis::Vertical,
            gap,
            children,
        }
    }

    /// The cells and the gaps along the stack, and the largest cell across it.
    #[must_use]
    pub fn size(self) -> GuiExtent {
        self.spread().size()
    }

    /// The bounds of the children, in order, for the stack at (`x`, `y`).
    pub fn rects(self, x: i32, y: i32) -> impl Iterator<Item = GuiRect> + 'a {
        self.spread().rects(x, y)
    }

    fn spread(self) -> EqualGaps<'a> {
        let gaps = i32::try_from(self.children.len().saturating_sub(1)).unwrap_or(i32::MAX);
        EqualGaps {
            axis: self.axis,
            length: equal_gaps::cell_lengths(self.children, self.axis)
                .saturating_add(self.gap.saturating_mul(gaps)),
            min_cross: 0,
            children: self.children,
        }
    }
}
