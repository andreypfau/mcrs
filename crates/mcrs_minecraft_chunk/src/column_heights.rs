use crate::{entries_per_long, packed_len};
use mcrs_minecraft_core::SectionPos;

/// One packed Y scalar over the 16x16 column footprint, indexed by `(x, z)` in
/// `0..16` each. The stored value is `1 + y` of the topmost block satisfying the
/// map's predicate, or `min_y` when the column holds no such block.
#[derive(Debug, Clone)]
pub struct ColumnHeights {
    longs: Vec<u64>,
    bits: u32,
    height: u32,
    min_y: i32,
}

impl ColumnHeights {
    pub fn new(height: u32, min_y: i32) -> Self {
        // stored value range is [0, height]
        let bits = (u32::BITS - height.leading_zeros()).max(1);
        Self {
            longs: vec![0; packed_len(bits, SectionPos::AREA)],
            bits,
            height,
            min_y,
        }
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    /// One past the highest Y this map can name.
    pub fn max_y(&self) -> i32 {
        self.min_y + self.height as i32
    }

    #[inline]
    fn index(x: usize, z: usize) -> usize {
        debug_assert!(
            x < SectionPos::SIZE && z < SectionPos::SIZE,
            "ColumnHeights index ({x}, {z}) out of range"
        );
        (z & SectionPos::MASK) * SectionPos::SIZE + (x & SectionPos::MASK)
    }

    #[inline]
    fn slot(&self, x: usize, z: usize) -> (usize, u32, u64) {
        let index = Self::index(x, z);
        let per_long = entries_per_long(self.bits);
        let shift = (index % per_long) as u32 * self.bits;
        (index / per_long, shift, (1u64 << self.bits) - 1)
    }

    pub fn get(&self, x: usize, z: usize) -> i32 {
        let (word, shift, mask) = self.slot(x, z);
        ((self.longs[word] >> shift) & mask) as i32 + self.min_y
    }

    pub fn set(&mut self, x: usize, z: usize, y: i32) {
        debug_assert!(
            y >= self.min_y && y <= self.max_y(),
            "ColumnHeights::set y={y} outside [{min}, {max}]",
            min = self.min_y,
            max = self.max_y(),
        );
        let rel = (y - self.min_y).clamp(0, self.height as i32);
        let (word, shift, mask) = self.slot(x, z);
        self.longs[word] = (self.longs[word] & !(mask << shift)) | ((rel as u64) << shift);
    }

    pub fn raw_longs(&self) -> &[u64] {
        &self.longs
    }

    pub fn bits(&self) -> u32 {
        self.bits
    }
}
