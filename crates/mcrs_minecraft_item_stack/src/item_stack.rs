use alloc::vec::Vec;

use crate::Count;
use crate::ItemComponent;
use crate::Kind;
use crate::PatchEntry;
use crate::patch_key::PatchKey;

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct ItemStack {
    pub kind: Kind,
    pub count: Count,
    patch: Vec<PatchEntry>,
}

impl ItemStack {
    #[must_use]
    pub fn new(kind: Kind, count: Count) -> Self {
        Self {
            kind,
            count,
            patch: Vec::new(),
        }
    }

    #[must_use]
    pub fn set<T: ItemComponent>(mut self, value: T) -> Self {
        self.put(T::patch_key(), Some(value.into_entry()));
        self
    }

    #[must_use]
    pub fn unset<T: ItemComponent>(mut self) -> Self {
        self.put(T::patch_key(), T::builtin_key().map(PatchEntry::Removal));
        self
    }

    #[must_use]
    pub fn patch(&self) -> &[PatchEntry] {
        &self.patch
    }

    #[must_use]
    pub fn mergeable(&self, other: &Self) -> bool {
        self.kind == other.kind && self.patch == other.patch
    }

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
        match entry {
            Some(entry) => self.put_entry(entry),
            None => {
                if let Ok(index) = self
                    .patch
                    .binary_search_by(|present| present.key().cmp(&key))
                {
                    self.patch.remove(index);
                }
            }
        }
    }
}
