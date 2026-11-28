use core::hash::Hash;

use rustc_hash::FxHashMap;

pub(super) struct Interner<T> {
    values: Vec<T>,
    index: FxHashMap<T, u16>,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            index: FxHashMap::default(),
        }
    }
}

impl<T: Clone + Eq + Hash> Interner<T> {
    pub(super) fn intern(&mut self, value: &T) -> u16 {
        if let Some(&index) = self.index.get(value) {
            return index;
        }
        let index = self.values.len() as u16;
        self.values.push(value.clone());
        self.index.insert(value.clone(), index);
        index
    }

    pub(super) fn into_values(self) -> Vec<T> {
        self.values
    }
}
