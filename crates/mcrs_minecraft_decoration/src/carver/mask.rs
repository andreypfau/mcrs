use fixedbitset::FixedBitSet;

/// The positions one carver pass freed, one bit per block, with the vertical in
/// the low bits so a tunnel or a shaft becomes a long run of set bits.
///
/// `width` is the side of the covered area in blocks: one chunk today, a whole
/// region once sources are enumerated per region.
pub struct CarvingMask {
    width: i32,
    min_y: i32,
    height: i32,
    bits: FixedBitSet,
}

impl CarvingMask {
    pub fn new(width: i32, min_y: i32, max_y: i32) -> Self {
        let height = max_y - min_y + 1;
        let width = width.max(0);
        CarvingMask {
            width,
            min_y,
            height,
            bits: FixedBitSet::with_capacity((width * width * height).max(0) as usize),
        }
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn clear(&mut self) {
        self.bits.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_clear()
    }

    #[inline]
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        if !(0..self.width).contains(&x)
            || !(0..self.width).contains(&z)
            || !(0..self.height).contains(&(y - self.min_y))
        {
            return None;
        }
        Some(((y - self.min_y) + (z + x * self.width) * self.height) as usize)
    }

    /// Positions outside the covered area are dropped, which is what clipping
    /// the ellipsoid bounds to the area used to do.
    #[inline]
    pub fn carve(&mut self, x: i32, y: i32, z: i32) {
        if let Some(index) = self.index(x, y, z) {
            self.bits.insert(index);
        }
    }

    #[inline]
    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        self.index(x, y, z).is_some_and(|index| self.bits[index])
    }

    /// Hand each column's carved runs to `visit` as `(x, z, bottom_y, top_y)`,
    /// inclusive. A run that spans several columns is split at the boundary, so
    /// a consumer that tracks state down a column never carries it across one.
    pub fn visit(&self, mut visit: impl FnMut(i32, i32, i32, i32)) {
        let mut run: Option<(usize, usize)> = None;
        for index in self.bits.ones() {
            match run {
                Some((start, end)) if index == end + 1 => run = Some((start, index)),
                Some((start, end)) => {
                    self.visit_segment(&mut visit, start, end);
                    run = Some((index, index));
                }
                None => run = Some((index, index)),
            }
        }
        if let Some((start, end)) = run {
            self.visit_segment(&mut visit, start, end);
        }
    }

    fn visit_segment(&self, visit: &mut impl FnMut(i32, i32, i32, i32), start: usize, end: usize) {
        let height = self.height as usize;
        for column in start / height..=end / height {
            let base = column * height;
            let x = column as i32 / self.width;
            let z = column as i32 % self.width;
            let bottom = start.saturating_sub(base).min(height - 1) as i32 + self.min_y;
            let top = (end - base).min(height - 1) as i32 + self.min_y;
            visit(x, z, bottom, top);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(mask: &CarvingMask) -> Vec<(i32, i32, i32, i32)> {
        let mut out = Vec::new();
        mask.visit(|x, z, bottom, top| out.push((x, z, bottom, top)));
        out
    }

    #[test]
    fn a_shaft_is_one_run() {
        let mut mask = CarvingMask::new(16, 0, 127);
        for y in 30..=40 {
            mask.carve(3, y, 9);
        }
        assert_eq!(runs(&mask), vec![(3, 9, 30, 40)]);
    }

    #[test]
    fn a_gap_splits_the_column() {
        let mut mask = CarvingMask::new(16, 0, 127);
        mask.carve(3, 30, 9);
        mask.carve(3, 31, 9);
        mask.carve(3, 40, 9);
        assert_eq!(runs(&mask), vec![(3, 9, 30, 31), (3, 9, 40, 40)]);
    }

    /// Columns are contiguous in the index, so a run that fills one column's top
    /// and the next column's bottom must still arrive as two per-column visits.
    #[test]
    fn a_run_across_the_column_boundary_is_split() {
        let mut mask = CarvingMask::new(16, 0, 7);
        for y in 5..=7 {
            mask.carve(0, y, 0);
        }
        for y in 0..=2 {
            mask.carve(0, y, 1);
        }
        assert_eq!(runs(&mask), vec![(0, 0, 5, 7), (0, 1, 0, 2)]);
    }

    #[test]
    fn out_of_range_positions_are_dropped() {
        let mut mask = CarvingMask::new(16, 1, 119);
        mask.carve(-1, 50, 0);
        mask.carve(16, 50, 0);
        mask.carve(0, 0, 0);
        mask.carve(0, 120, 0);
        assert!(mask.is_empty());
        mask.carve(0, 1, 0);
        assert!(mask.contains(0, 1, 0));
        assert!(!mask.contains(0, 2, 0));
    }

    #[test]
    fn a_region_wide_mask_keeps_its_own_columns_apart() {
        let mut mask = CarvingMask::new(32, 0, 127);
        mask.carve(31, 64, 17);
        assert!(mask.contains(31, 64, 17));
        assert_eq!(runs(&mask), vec![(31, 17, 64, 64)]);
    }
}
