use alloc::vec::Vec;

use crate::Count;
use crate::ItemComponent;
use crate::Kind;
use crate::PatchEntry;
use crate::patch_key::PatchKey;

/// An item, a count and a component patch.
///
/// The patch has at most one entry for each component type and keeps one order of its entries.
/// Thus two stacks are equal when they have the same item, count and patch entries, whatever the
/// order of the calls that made them.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct ItemStack {
    pub kind: Kind,
    pub count: Count,
    patch: Vec<PatchEntry>,
}

impl ItemStack {
    /// A stack with an empty patch.
    #[must_use]
    pub fn new(kind: Kind, count: Count) -> Self {
        Self {
            kind,
            count,
            patch: Vec::new(),
        }
    }

    /// Puts a value entry for `T` in the patch. It replaces the entry that `T` had.
    #[must_use]
    pub fn set<T: ItemComponent>(mut self, value: T) -> Self {
        self.put(T::patch_key(), Some(value.into_entry()));
        self
    }

    /// Puts a removal entry for `T` in the patch. For a custom component, which no prototype
    /// has, it drops the entry of `T`.
    #[must_use]
    pub fn unset<T: ItemComponent>(mut self) -> Self {
        self.put(T::patch_key(), T::removal());
        self
    }

    /// The patch entries: the built-in types in the order of their type ids, then the types that
    /// this crate does not decode in the order of their identifiers, then those that a wire reader
    /// kept in the order of their type ids, then the custom types in the order of their keys.
    #[must_use]
    pub fn patch(&self) -> &[PatchEntry] {
        &self.patch
    }

    /// True when the two stacks have the same item and equal patches. The count has no part.
    #[must_use]
    pub fn mergeable(&self, other: &Self) -> bool {
        self.kind == other.kind && self.patch == other.patch
    }

    /// Puts `entry` in the patch. It replaces the entry of the same type.
    #[doc(hidden)]
    #[must_use]
    pub fn with_entry(mut self, entry: PatchEntry) -> Self {
        self.put_entry(entry);
        self
    }

    pub(crate) fn put_entry(&mut self, entry: PatchEntry) {
        let found = self
            .patch
            .binary_search_by(|present| present.key().cmp(&entry.key()));
        match found {
            Ok(index) => {
                if let Some(present) = self.patch.get_mut(index) {
                    *present = entry;
                }
            }
            Err(index) => self.patch.insert(index, entry),
        }
    }

    fn put(&mut self, key: PatchKey<'static>, entry: Option<PatchEntry>) {
        let found = self
            .patch
            .binary_search_by(|present| present.key().cmp(&key));
        match (found, entry) {
            (Ok(index), Some(entry)) => {
                if let Some(present) = self.patch.get_mut(index) {
                    *present = entry;
                }
            }
            (Ok(index), None) => {
                self.patch.remove(index);
            }
            (Err(index), Some(entry)) => self.patch.insert(index, entry),
            (Err(_), None) => {}
        }
    }
}
