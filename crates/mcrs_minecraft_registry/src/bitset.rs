use crate::StaticId;
use fixedbitset::FixedBitSet;
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

/// After `TagLoader::freeze()` each tag's membership set is stored as a
/// `BitSet` — membership tests become a single array index + bitmask
/// instead of a `HashSet` hash probe.
pub struct BitSet<I> {
    bits: FixedBitSet,
    _marker: PhantomData<fn() -> I>,
}

/// Bitset over a static registry's typed ids.
pub type IdBitSet<T> = BitSet<StaticId<T>>;

/// Bitset over a dynamic registry's dense `u32` ids.
pub type RawBitSet = BitSet<u32>;

impl<I> Clone for BitSet<I> {
    fn clone(&self) -> Self {
        Self {
            bits: self.bits.clone(),
            _marker: PhantomData,
        }
    }
}

impl<I: TagId> BitSet<I> {
    pub fn with_capacity(cap: u32) -> Self {
        Self {
            bits: FixedBitSet::with_capacity(cap as usize),
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, id: I) {
        self.bits.grow_and_insert(id.raw() as usize);
    }

    #[inline]
    pub fn contains(&self, id: I) -> bool {
        self.bits.contains(id.raw() as usize)
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.bits.count_ones(..) as u32
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bits.is_clear()
    }

    pub fn iter(&self) -> impl Iterator<Item = I> + '_ {
        self.bits.ones().map(|raw| I::from_raw(raw as u32))
    }

    pub fn from_hash_set(set: &HashSet<I>, capacity: u32) -> Self {
        let mut bs = Self::with_capacity(capacity);
        for &id in set {
            bs.insert(id);
        }
        bs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_grows_and_iterates_in_order() {
        let mut bs = RawBitSet::with_capacity(8);
        for raw in [200, 3, 64, 3] {
            bs.insert(raw);
        }
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(200) && !bs.contains(9999));
        assert_eq!(bs.iter().collect::<Vec<_>>(), vec![3, 64, 200]);
    }
}
