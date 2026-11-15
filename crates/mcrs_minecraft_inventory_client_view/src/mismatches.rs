use alloc::vec::Vec;

use crate::Agreement;

/// The result of a comparison: the entries and the cursor to send again. Keep one and lend it to
/// each comparison: after the largest menu, a comparison makes no allocation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mismatches {
    pub(crate) entries: Vec<usize>,
    pub(crate) cursor: Agreement,
}

impl Mismatches {
    /// The menu index of each entry that does not match, in ascending order.
    #[must_use]
    pub fn entries(&self) -> &[usize] {
        &self.entries
    }

    #[must_use]
    pub fn cursor(&self) -> Agreement {
        self.cursor
    }

    /// True when each entry and the cursor match.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.cursor == Agreement::Matches
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.cursor = Agreement::Matches;
    }
}
