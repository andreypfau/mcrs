use crate::{PackedBitStorage, bits_needed_for};
use mcrs_voxel_math::chunk_pos::BLOCKS;

/// One packed Y scalar over the 16x16 column footprint, indexed by `(x, z)` in
/// `0..16` each. The stored value is `1 + y` of the topmost block satisfying the
/// map's predicate, or `min_y` when the column holds no such block.
#[derive(Debug, Clone)]
pub struct ColumnHeights {
    store: PackedBitStorage,
    height: u32,
    min_y: i32,
}

impl ColumnHeights {
    pub fn new(height: u32, min_y: i32) -> Self {
        let max_value = height; // stored value range is [0, height]
        Self {
            store: PackedBitStorage::with_bits(BLOCKS::AREA, bits_needed_for(max_value), max_value),
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
            x < BLOCKS::SIZE && z < BLOCKS::SIZE,
            "ColumnHeights index ({x}, {z}) out of range"
        );
        (z & BLOCKS::MASK) * BLOCKS::SIZE + (x & BLOCKS::MASK)
    }

    pub fn get(&self, x: usize, z: usize) -> i32 {
        self.store.get(Self::index(x, z)) as i32 + self.min_y
    }

    pub fn set(&mut self, x: usize, z: usize, y: i32) {
        debug_assert!(
            y >= self.min_y && y <= self.max_y(),
            "ColumnHeights::set y={y} outside [{min}, {max}]",
            min = self.min_y,
            max = self.max_y(),
        );
        let rel = (y - self.min_y).clamp(0, self.height as i32);
        self.store.set(Self::index(x, z), rel as u32);
    }

    pub fn raw_longs(&self) -> &[u64] {
        self.store.raw_longs()
    }

    pub fn storage(&self) -> &PackedBitStorage {
        &self.store
    }
}

