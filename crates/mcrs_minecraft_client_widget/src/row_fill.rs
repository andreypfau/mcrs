use core::num::NonZeroU32;

use crate::child::Child;
use crate::grid_child::GridChild;

/// Puts the children of a grid in rows of a fixed number of columns, in the order of addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowFill {
    columns: NonZeroU32,
    next: u32,
}

impl RowFill {
    /// A fill of rows of `columns` columns; 0 counts as 1.
    #[must_use]
    pub fn new(columns: u16) -> Self {
        Self {
            columns: NonZeroU32::new(u32::from(columns)).unwrap_or(NonZeroU32::MIN),
            next: 0,
        }
    }

    /// The next child with the column span `column_span`, at the start of the next row when it
    /// does not fit in the current row; a span of 0 counts as 1.
    pub fn add(&mut self, child: Child, column_span: u16) -> GridChild {
        let column_span = column_span.max(1);
        let span = u32::from(column_span);
        if (self.next % self.columns).saturating_add(span) > self.columns.get() {
            self.next = self
                .next
                .checked_next_multiple_of(self.columns.get())
                .unwrap_or(u32::MAX);
        }
        let row = u16::try_from(self.next / self.columns).unwrap_or(u16::MAX);
        let column = u16::try_from(self.next % self.columns).unwrap_or(u16::MAX);
        self.next = self.next.saturating_add(span);
        GridChild {
            child,
            row,
            column,
            row_span: 1,
            column_span,
        }
    }
}
