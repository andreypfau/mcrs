use crate::registry::StaticId;
use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;

/// A dense index into the registry a tag is defined over.
pub trait TagId: Copy + Eq + Hash + Send + Sync + 'static {
    fn raw(self) -> u32;
    fn from_raw(raw: u32) -> Self;
}

impl TagId for u32 {
    #[inline]
    fn raw(self) -> u32 {
        self
    }

    #[inline]
    fn from_raw(raw: u32) -> Self {
        raw
    }
}

impl<T: 'static> TagId for StaticId<T> {
    #[inline]
    fn raw(self) -> u32 {
        self.id
    }

    #[inline]
    fn from_raw(raw: u32) -> Self {
        StaticId::new(raw)
    }
}

/// A compact bitset indexed by `I`, backed by `Vec<u64>`.
///
/// After `TagLoader::freeze()` each tag's membership set is stored as a
/// `BitSet` — membership tests become a single array index + bitmask
/// instead of a `HashSet` hash probe.
pub struct BitSet<I> {
    words: Vec<u64>,
    len: u32,
    _marker: PhantomData<fn() -> I>,
}

/// Bitset over a static registry's typed ids.
pub type IdBitSet<T> = BitSet<StaticId<T>>;

/// Bitset over a dynamic registry's dense `u32` ids.
pub type RawBitSet = BitSet<u32>;

impl<I> Clone for BitSet<I> {
    fn clone(&self) -> Self {
        Self {
            words: self.words.clone(),
            len: self.len,
            _marker: PhantomData,
        }
    }
}

impl<I: TagId> BitSet<I> {
    /// Create an empty bitset with room for ids in `0..cap`.
    pub fn with_capacity(cap: u32) -> Self {
        Self {
            words: vec![0u64; word_count(cap)],
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Set the bit for `id`. No-op if already set.
    pub fn insert(&mut self, id: I) {
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
    pub fn contains(&self, id: I) -> bool {
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

    /// Iterate over all set ids using `trailing_zeros()` for efficient
    /// scanning of sparse words.
    pub fn iter(&self) -> BitSetIter<'_, I> {
        BitSetIter {
            words: &self.words,
            word_idx: 0,
            current: self.words.first().copied().unwrap_or(0),
            _marker: PhantomData,
        }
    }

    /// Build from a `HashSet<I>` with the given capacity (typically
    /// `registry.len()`). Bridge from the pre-freeze representation.
    pub fn from_hash_set(set: &HashSet<I>, capacity: u32) -> Self {
        let mut bs = Self::with_capacity(capacity);
        for &id in set {
            bs.insert(id);
        }
        bs
    }
}

pub struct BitSetIter<'a, I> {
    words: &'a [u64],
    word_idx: usize,
    current: u64,
    _marker: PhantomData<fn() -> I>,
}

impl<I: TagId> Iterator for BitSetIter<'_, I> {
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let tz = self.current.trailing_zeros();
                self.current &= self.current - 1;
                return Some(I::from_raw((self.word_idx * 64 + tz as usize) as u32));
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
    (bits as usize).div_ceil(64)
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

    // ── RawBitSet (same storage, plain `u32` ids) ──

    #[test]
    fn raw_bitset_empty() {
        let bs = RawBitSet::with_capacity(128);
        assert_eq!(bs.len(), 0);
        assert!(bs.is_empty());
        assert!(!bs.contains(0));
    }

    #[test]
    fn raw_bitset_insert_and_contains() {
        let mut bs = RawBitSet::with_capacity(256);
        bs.insert(0);
        bs.insert(63);
        bs.insert(64);
        bs.insert(200);
        assert_eq!(bs.len(), 4);
        assert!(bs.contains(0));
        assert!(bs.contains(63));
        assert!(bs.contains(64));
        assert!(bs.contains(200));
        assert!(!bs.contains(1));
        assert!(!bs.contains(65));
    }

    #[test]
    fn raw_bitset_duplicate_insert() {
        let mut bs = RawBitSet::with_capacity(64);
        bs.insert(10);
        assert_eq!(bs.len(), 1);
        bs.insert(10);
        assert_eq!(bs.len(), 1);
    }

    #[test]
    fn raw_bitset_from_hash_set() {
        let mut set = HashSet::new();
        set.insert(5u32);
        set.insert(10);
        set.insert(100);
        let bs = RawBitSet::from_hash_set(&set, 128);
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(5));
        assert!(bs.contains(10));
        assert!(bs.contains(100));
        assert!(!bs.contains(0));
    }

    #[test]
    fn raw_bitset_iter() {
        let mut bs = RawBitSet::with_capacity(256);
        bs.insert(3);
        bs.insert(7);
        bs.insert(128);
        let got: Vec<u32> = bs.iter().collect();
        assert_eq!(got, vec![3, 7, 128]);
    }

    #[test]
    fn raw_bitset_contains_out_of_range() {
        let bs = RawBitSet::with_capacity(64);
        assert!(!bs.contains(9999));
    }

    #[test]
    fn raw_bitset_zero_capacity() {
        let bs = RawBitSet::with_capacity(0);
        assert!(bs.is_empty());
        assert!(!bs.contains(0));
        assert_eq!(bs.iter().count(), 0);
    }
}
