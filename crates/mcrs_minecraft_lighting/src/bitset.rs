/// 256-bit fixed-size bitset backed by `[u64; 4]`.
///
/// Used as the per-XZ closure mask in the incremental heightmap scan. The
/// scan owns one bitset per heightmap variant (`world_surface`,
/// `motion_blocking`) and the "all 256 XZ columns closed" hot-path query
/// reduces to `is_full()`, a 4-way AND-tree against `u64::MAX`.
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct BitSet256([u64; 4]);

impl BitSet256 {
    #[inline]
    pub fn is_set(&self, idx: usize) -> bool {
        debug_assert!(idx < 256);
        (self.0[idx >> 6] >> (idx & 63)) & 1 == 1
    }

    #[inline]
    pub fn set(&mut self, idx: usize) {
        debug_assert!(idx < 256);
        self.0[idx >> 6] |= 1u64 << (idx & 63);
    }

    /// True iff every bit is set.
    #[inline]
    pub fn is_full(&self) -> bool {
        (self.0[0] & self.0[1] & self.0[2] & self.0[3]) == u64::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_closure_tracking() {
        let mut b = BitSet256::default();
        assert!(!b.is_full(), "default bitset must report not-full");
        for idx in 0..256 {
            assert!(!b.is_set(idx), "default bitset must report bit {idx} unset");
        }

        for &idx in &[0usize, 63, 64, 127, 128, 191, 192, 255] {
            assert!(!b.is_set(idx));
            b.set(idx);
            assert!(b.is_set(idx), "bit {idx} must be set after `set`");
        }
        assert!(!b.is_full(), "partial bitset must not report full");

        // Setting all 256 indices in arbitrary order must produce is_full() == true.
        let mut all = BitSet256::default();
        for idx in (0..256).rev() {
            all.set(idx);
        }
        assert!(all.is_full(), "bitset with every bit set must report full");

        // High-word boundary: setting bits 0..=254 must leave is_full() == false.
        let mut almost = BitSet256::default();
        for idx in 0..255 {
            almost.set(idx);
        }
        assert!(
            !almost.is_full(),
            "bitset missing only the top-most bit must NOT report full"
        );
        assert!(almost.is_set(254));
        assert!(!almost.is_set(255));
    }

    #[test]
    fn full_bitset_is_full() {
        let mut b = BitSet256::default();
        for idx in 0..256 {
            b.set(idx);
        }
        assert!(b.is_full());
    }

    #[test]
    fn idempotent_set() {
        let mut b = BitSet256::default();
        b.set(42);
        b.set(42);
        assert!(b.is_set(42));
    }
}
