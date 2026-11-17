use std::hash::Hash;

use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Coverage<P> {
    counts: FxHashMap<P, u32>,
}

impl<P> Default for Coverage<P> {
    fn default() -> Self {
        Self {
            counts: FxHashMap::default(),
        }
    }
}

impl<P: Copy + Eq + Hash> Coverage<P> {
    pub fn contains(&self, pos: P) -> bool {
        self.counts.contains_key(&pos)
    }

    pub fn iter(&self) -> impl Iterator<Item = P> + '_ {
        self.counts.keys().copied()
    }

    pub fn raise(&mut self, positions: impl IntoIterator<Item = P>, mut entered: impl FnMut(P)) {
        for pos in positions {
            let count = self.counts.entry(pos).or_insert(0);
            *count = count.saturating_add(1);
            if *count == 1 {
                entered(pos);
            }
        }
    }

    pub fn lower(&mut self, positions: impl IntoIterator<Item = P>, mut left: impl FnMut(P)) {
        for pos in positions {
            let Some(count) = self.counts.get_mut(&pos) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.counts.remove(&pos);
                left(pos);
            }
        }
    }
}
