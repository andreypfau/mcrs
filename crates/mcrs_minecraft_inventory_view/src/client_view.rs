use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;

use crate::Agreement;
use crate::Mismatches;
use crate::SlotView;

/// The view of each entry of one menu, in the order of the menu index, and of the cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientView {
    entries: Vec<SlotView>,
    cursor: SlotView,
}

impl ClientView {
    /// A view of `len` entries and the cursor, each not known.
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

    /// The view of the entry at `index`, or `None` when the menu has no such entry.
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

    /// Writes into `mismatches` each entry of `entries` and the cursor that do not match this
    /// view. `entries` is the stack of each entry in the order of the menu index. An entry past
    /// the end of this view has no view and never matches. The view does not change: the caller
    /// records each stack it sends.
    pub fn compare<'stack, L: NameLookup>(
        &self,
        entries: impl IntoIterator<Item = Option<&'stack ItemStack>>,
        cursor: Option<&ItemStack>,
        names: &L,
        scratch: &mut HashScratch,
        mismatches: &mut Mismatches,
    ) {
        mismatches.clear();
        for (index, stack) in entries.into_iter().enumerate() {
            let matches = self
                .entries
                .get(index)
                .is_some_and(|view| view.matches(stack, names, scratch));
            if !matches {
                mismatches.entries.push(index);
            }
        }
        if !self.cursor.matches(cursor, names, scratch) {
            mismatches.cursor = Agreement::Differs;
        }
    }
}
