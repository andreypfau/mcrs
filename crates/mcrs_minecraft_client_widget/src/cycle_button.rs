/// The selected value of a cycle button among its `count` values (WID-CYC-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleButton {
    index: usize,
    count: usize,
}

impl CycleButton {
    /// The value `index` of `count` values; none unless `index < count`.
    #[must_use]
    pub const fn new(count: usize, index: usize) -> Option<Self> {
        if index < count {
            Some(Self { index, count })
        } else {
            None
        }
    }

    /// A Boolean cycle button, whose values are true at index 0, then false.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        Self {
            index: if value { 0 } else { 1 },
            count: 2,
        }
    }

    /// The index of the selected value.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn select_next(&mut self) {
        self.index = self
            .index
            .checked_add(1)
            .filter(|next| *next < self.count)
            .unwrap_or(0);
    }

    pub(crate) fn select_previous(&mut self) {
        self.index = self
            .index
            .checked_sub(1)
            .unwrap_or(self.count.saturating_sub(1));
    }
}
