use fixedbitset::FixedBitSet;

const BITS: usize = usize::BITS as usize;

#[derive(PartialEq, Eq)]
pub struct CarvingMask {
    min_y: i32,
    height: i32,
    bits: FixedBitSet,
}

impl CarvingMask {
    const WIDTH: i32 = 16;

    pub fn new(min_y: i32, max_y: i32) -> Self {
        let height = max_y - min_y + 1;
        CarvingMask {
            min_y,
            height,
            bits: FixedBitSet::with_capacity((Self::WIDTH * Self::WIDTH * height).max(0) as usize),
        }
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_clear()
    }

    #[inline]
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        if !(0..Self::WIDTH).contains(&x)
            || !(0..Self::WIDTH).contains(&z)
            || !(0..self.height).contains(&(y - self.min_y))
        {
            return None;
        }
        Some(((y - self.min_y) + (z + x * Self::WIDTH) * self.height) as usize)
    }

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

    #[must_use]
    pub fn highest_carved(&self, x: i32, z: i32, bottom: i32, top: i32) -> Option<i32> {
        let bottom = bottom.max(self.min_y);
        let top = top.min(self.max_y());
        if bottom > top {
            return None;
        }
        let low = self.index(x, bottom, z)?;
        let high = self.index(x, top, z)?;
        let words = self.bits.as_slice();
        let mut word = high / BITS;
        let mut mask = usize::MAX >> (BITS - 1 - high % BITS);
        loop {
            let value = words.get(word).copied().unwrap_or(0) & mask;
            if value != 0 {
                let carved = word * BITS + (BITS - 1 - value.leading_zeros() as usize);
                return (carved >= low).then(|| top - (high - carved) as i32);
            }
            if word * BITS <= low {
                return None;
            }
            word -= 1;
            mask = usize::MAX;
        }
    }

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
            let x = column as i32 / Self::WIDTH;
            let z = column as i32 % Self::WIDTH;
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
    fn runs_split_at_gaps_and_column_boundaries() {
        type Run = (i32, i32, i32, i32);
        let cases: &[(i32, &[Run], &[Run])] = &[
            (127, &[(3, 9, 30, 40)], &[(3, 9, 30, 40)]),
            (
                127,
                &[(3, 9, 30, 31), (3, 9, 40, 40)],
                &[(3, 9, 30, 31), (3, 9, 40, 40)],
            ),
            (
                7,
                &[(0, 0, 5, 7), (0, 1, 0, 2)],
                &[(0, 0, 5, 7), (0, 1, 0, 2)],
            ),
        ];
        for &(top, carved, expected) in cases {
            let mut mask = CarvingMask::new(0, top);
            for &(x, z, bottom, top) in carved {
                for y in bottom..=top {
                    mask.carve(x, y, z);
                }
            }
            assert_eq!(runs(&mask), expected);
        }
    }

    #[test]
    fn highest_carved_matches_a_scan_of_each_block() {
        let mut mask = CarvingMask::new(-5, 194);
        for (x, z, y) in [
            (0, 0, 58),
            (0, 0, 59),
            (0, 1, -5),
            (0, 1, 122),
            (0, 1, 194),
            (1, 0, 0),
        ] {
            mask.carve(x, y, z);
        }
        let empty = CarvingMask::new(-5, 194);
        let mut full = CarvingMask::new(-5, 194);
        for y in -5..=194 {
            full.carve(0, y, 1);
        }
        let ranges = [
            (-5, 194),
            (-64, 400),
            (-5, 58),
            (-5, 57),
            (59, 59),
            (60, 121),
            (60, 122),
            (-4, 121),
            (-5, -5),
            (-4, 194),
            (123, 193),
            (100, 50),
        ];
        for mask in [&mask, &empty, &full] {
            for (x, z) in [(0, 0), (0, 1), (1, 0), (15, 15)] {
                for (bottom, top) in ranges {
                    let expected = (bottom.max(-5)..=top.min(194))
                        .rev()
                        .find(|&y| mask.contains(x, y, z));
                    assert_eq!(
                        mask.highest_carved(x, z, bottom, top),
                        expected,
                        "column {x} {z} from {bottom} to {top}"
                    );
                }
            }
        }
    }

    #[test]
    fn out_of_range_positions_are_dropped() {
        let mut mask = CarvingMask::new(1, 119);
        mask.carve(-1, 50, 0);
        mask.carve(16, 50, 0);
        mask.carve(0, 0, 0);
        mask.carve(0, 120, 0);
        assert!(mask.is_empty());
        mask.carve(0, 1, 0);
        assert!(mask.contains(0, 1, 0));
        assert!(!mask.contains(0, 2, 0));
    }
}
