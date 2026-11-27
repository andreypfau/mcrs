use core::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditBoxRun {
    pub range: Range<usize>,
    pub color: u32,
}

impl EditBoxRun {
    #[must_use]
    pub const fn new(range: Range<usize>, color: u32) -> Self {
        Self { range, color }
    }
}
