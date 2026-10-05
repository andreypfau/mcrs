use crate::Id;
use fixedbitset::FixedBitSet;
use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;

/// A dense index into the registry a tag is defined over.
pub trait DenseId: Copy + Eq + Hash + Send + Sync + 'static {
    fn raw(self) -> u16;
    fn from_raw(raw: u16) -> Self;
}

impl<R: 'static> DenseId for Id<R> {
    #[inline]
    fn raw(self) -> u16 {
        self.number()
    }

    #[inline]
    fn from_raw(raw: u16) -> Self {
        Id::from_number(raw)
    }
}

/// A membership set stored as a bitset: a test is a single array index and
/// bitmask instead of a `HashSet` hash probe.
pub struct BitSet<I> {
    bits: FixedBitSet,
    _marker: PhantomData<fn() -> I>,
}

impl<I> Clone for BitSet<I> {
    fn clone(&self) -> Self {
        Self {
            bits: self.bits.clone(),
            _marker: PhantomData,
        }
    }
}

impl<I: DenseId> BitSet<I> {
    pub fn with_capacity(cap: u32) -> Self {
        Self {
            bits: FixedBitSet::with_capacity(cap as usize),
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, id: I) {
        self.bits.grow_and_insert(usize::from(id.raw()));
    }

    #[inline]
    pub fn contains(&self, id: I) -> bool {
        self.bits.contains(usize::from(id.raw()))
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
        self.bits
            .ones()
            .map(|raw| I::from_raw(u16::try_from(raw).expect("a bitset holds only inserted ids")))
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

    struct Entry;

    #[test]
    fn insert_grows_and_iterates_in_order() {
        let id = Id::<Entry>::from_raw;
        let mut bs = BitSet::<Id<Entry>>::with_capacity(8);
        for raw in [200, 3, 64, 3] {
            bs.insert(id(raw));
        }
        assert_eq!(bs.len(), 3);
        assert!(bs.contains(id(200)) && !bs.contains(id(9999)));
        assert_eq!(bs.iter().collect::<Vec<_>>(), vec![id(3), id(64), id(200)]);
    }
}
