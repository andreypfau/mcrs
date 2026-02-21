use crate::registry::StaticId;
use std::marker::PhantomData;

/// A compact bitset indexed by `StaticId<T>`, backed by `Vec<u64>`.
///
/// After `StaticTags::freeze()` each tag's membership set is stored as an
/// `IdBitSet` — membership tests become a single array index + bitmask
/// instead of a `HashSet` hash probe.
pub struct IdBitSet<T> {
    words: Vec<u64>,
    len: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> IdBitSet<T> {
    /// Create an empty bitset with room for IDs in `0..cap`.
    pub fn with_capacity(cap: u32) -> Self {
        let num_words = word_count(cap);
        Self {
            words: vec![0u64; num_words],
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Set the bit for `id`. No-op if already set.
    pub fn insert(&mut self, id: StaticId<T>) {
        let raw = id.raw() as usize;
        let (word, bit) = (raw / 64, raw % 64);
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let mask = 1u64 << bit;
        if self.words[word] & mask == 0 {
            self.words[word] |= mask;
            self.len += 1;
        }
    }

    /// O(1) membership test: single array index + bitmask.
    #[inline]
    pub fn contains(&self, id: StaticId<T>) -> bool {
        let raw = id.raw() as usize;
        let (word, bit) = (raw / 64, raw % 64);
        word < self.words.len() && (self.words[word] & (1u64 << bit)) != 0
    }

    /// Number of set bits.
    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Iterate over all set `StaticId<T>` values using `trailing_zeros()` for
    /// efficient scanning of sparse words.
    pub fn iter(&self) -> IdBitSetIter<'_, T> {
        IdBitSetIter {
            words: &self.words,
            word_idx: 0,
            current: if self.words.is_empty() {
                0
            } else {
                self.words[0]
            },
            _marker: PhantomData,
        }
    }

    /// Build from a `HashSet<StaticId<T>>` with the given capacity (typically
    /// `registry.len()`). Bridge from the pre-freeze representation.
    pub fn from_hash_set(set: &std::collections::HashSet<StaticId<T>>, capacity: u32) -> Self {
        let mut bs = Self::with_capacity(capacity);
        for &id in set {
            bs.insert(id);
        }
        bs
    }
}

pub struct IdBitSetIter<'a, T> {
    words: &'a [u64],
    word_idx: usize,
    current: u64,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Iterator for IdBitSetIter<'_, T> {
    type Item = StaticId<T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let tz = self.current.trailing_zeros();
                // Clear the lowest set bit.
                self.current &= self.current - 1;
                let raw = (self.word_idx * 64 + tz as usize) as u32;
                return Some(StaticId::new(raw));
            }
            self.word_idx += 1;
            if self.word_idx >= self.words.len() {
                return None;
            }
            self.current = self.words[self.word_idx];
        }
    }
}

#[inline]
fn word_count(bits: u32) -> usize {
    ((bits as usize) + 63) / 64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Dummy type so we can create `StaticId<Dummy>` in tests.
    struct Dummy;

    fn id(raw: u32) -> StaticId<Dummy> {
        StaticId::new(raw)
    }

    #[test]
    fn empty_bitset() {
        let bs = IdBitSet::<Dummy>::with_capacity(128);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert!(!bs.contains(id(0)));
        assert!(!bs.contains(id(127)));
        assert_eq!(bs.iter().count(), 0);
    }

    #[test]
    fn insert_and_contains() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(256);
        bs.insert(id(0));
        bs.insert(id(1));
        bs.insert(id(63));
        bs.insert(id(64));
        bs.insert(id(200));

        assert!(bs.contains(id(0)));
        assert!(bs.contains(id(1)));
        assert!(bs.contains(id(63)));
        assert!(bs.contains(id(64)));
        assert!(bs.contains(id(200)));

        assert!(!bs.contains(id(2)));
        assert!(!bs.contains(id(62)));
        assert!(!bs.contains(id(65)));
        assert!(!bs.contains(id(199)));
        assert!(!bs.contains(id(255)));

        assert_eq!(bs.len(), 5);
        assert!(!bs.is_empty());
    }

    #[test]
    fn duplicate_insert_does_not_change_len() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(64);
        bs.insert(id(10));
        assert_eq!(bs.len(), 1);
        bs.insert(id(10));
        assert_eq!(bs.len(), 1);
    }

    #[test]
    fn insert_beyond_capacity_grows() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(8);
        bs.insert(id(200));
        assert!(bs.contains(id(200)));
        assert_eq!(bs.len(), 1);
    }

    #[test]
    fn iter_returns_sorted_ids() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(256);
        let expected = [3, 7, 63, 64, 65, 128, 200];
        for &raw in &expected {
            bs.insert(id(raw));
        }
        let got: Vec<u32> = bs.iter().map(|sid| sid.raw()).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn iter_empty() {
        let bs = IdBitSet::<Dummy>::with_capacity(64);
        assert_eq!(bs.iter().count(), 0);
    }

    #[test]
    fn iter_single_element() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(64);
        bs.insert(id(42));
        let got: Vec<u32> = bs.iter().map(|sid| sid.raw()).collect();
        assert_eq!(got, vec![42]);
    }

    #[test]
    fn iter_all_bits_in_word() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(64);
        for i in 0..64 {
            bs.insert(id(i));
        }
        assert_eq!(bs.len(), 64);
        let got: Vec<u32> = bs.iter().map(|sid| sid.raw()).collect();
        let expected: Vec<u32> = (0..64).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn from_hash_set_roundtrip() {
        let mut set = HashSet::new();
        set.insert(id(5));
        set.insert(id(10));
        set.insert(id(100));

        let bs = IdBitSet::from_hash_set(&set, 128);
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(id(5)));
        assert!(bs.contains(id(10)));
        assert!(bs.contains(id(100)));
        assert!(!bs.contains(id(0)));
        assert!(!bs.contains(id(50)));

        // Iterator should return the same ids.
        let mut iter_ids: Vec<u32> = bs.iter().map(|sid| sid.raw()).collect();
        iter_ids.sort();
        assert_eq!(iter_ids, vec![5, 10, 100]);
    }

    #[test]
    fn from_empty_hash_set() {
        let set = HashSet::new();
        let bs = IdBitSet::<Dummy>::from_hash_set(&set, 64);
        assert!(bs.is_empty());
        assert_eq!(bs.len(), 0);
        assert_eq!(bs.iter().count(), 0);
    }

    #[test]
    fn contains_out_of_range_returns_false() {
        let bs = IdBitSet::<Dummy>::with_capacity(64);
        // ID far beyond allocated words.
        assert!(!bs.contains(id(9999)));
    }

    #[test]
    fn zero_capacity() {
        let bs = IdBitSet::<Dummy>::with_capacity(0);
        assert!(bs.is_empty());
        assert!(!bs.contains(id(0)));
        assert_eq!(bs.iter().count(), 0);
    }

    #[test]
    fn word_boundary_ids() {
        let mut bs = IdBitSet::<Dummy>::with_capacity(256);
        // Insert at every word boundary.
        for w in 0..4 {
            bs.insert(id(w * 64));
            bs.insert(id(w * 64 + 63));
        }
        assert_eq!(bs.len(), 8);
        for w in 0..4 {
            assert!(bs.contains(id(w * 64)));
            assert!(bs.contains(id(w * 64 + 63)));
            assert!(!bs.contains(id(w * 64 + 1)));
        }
    }
}
