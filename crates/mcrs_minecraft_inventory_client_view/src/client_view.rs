use alloc::vec;
use alloc::vec::Vec;

use crate::SlotView;

#[derive(Clone, Debug, PartialEq)]
pub struct ClientView {
    entries: Vec<SlotView>,
    cursor: SlotView,
}

impl ClientView {
    #[must_use]
    pub fn new(len: usize) -> Self {
        Self {
            entries: vec![SlotView::Unknown; len],
            cursor: SlotView::Unknown,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&SlotView> {
        self.entries.get(index)
    }

    pub fn entry_mut(&mut self, index: usize) -> Option<&mut SlotView> {
        self.entries.get_mut(index)
    }

    #[must_use]
    pub fn cursor(&self) -> &SlotView {
        &self.cursor
    }

    pub fn cursor_mut(&mut self) -> &mut SlotView {
        &mut self.cursor
    }
}
