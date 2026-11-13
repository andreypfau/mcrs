use crate::arithmetic;
use crate::axis::Axis;
use crate::child::Child;

/// A child of a grid in its row and column; a span of 0 counts as 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridChild {
    pub child: Child,
    pub row: u16,
    pub column: u16,
    pub row_span: u16,
    pub column_span: u16,
}

impl GridChild {
    pub(crate) fn first_track(self, axis: Axis) -> i32 {
        i32::from(match axis {
            Axis::Horizontal => self.column,
            Axis::Vertical => self.row,
        })
    }

    pub(crate) fn span(self, axis: Axis) -> i32 {
        i32::from(match axis {
            Axis::Horizontal => self.column_span,
            Axis::Vertical => self.row_span,
        })
        .max(1)
    }

    pub(crate) fn share_in(self, axis: Axis, gap: i32, track: i32) -> Option<i32> {
        let span = self.span(axis);
        let index = track
            .saturating_sub(self.first_track(axis))
            .saturating_add(1);
        let total = self
            .child
            .cell_size()
            .along(axis)
            .saturating_sub(gap.saturating_mul(span.saturating_sub(1)));
        (1..=span)
            .contains(&index)
            .then(|| arithmetic::even_split(total, span, index))
    }
}
