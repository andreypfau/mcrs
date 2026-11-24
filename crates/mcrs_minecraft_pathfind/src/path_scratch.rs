use mcrs_minecraft_physics::{PathType, SectionLookup};
use mcrs_voxel::{BlockPos, SectionPos};
use rustc_hash::FxHashMap;

pub struct PathScratch<S> {
    pub(crate) block_types: FxHashMap<BlockPos, PathType>,
    pub(crate) mob_types: FxHashMap<BlockPos, PathType>,
    pub(crate) sections: [Option<(SectionPos, SectionLookup<S>)>; 64],
    pub(crate) next_section: usize,
}

impl<S> Default for PathScratch<S> {
    fn default() -> Self {
        Self {
            block_types: FxHashMap::default(),
            mob_types: FxHashMap::default(),
            sections: core::array::from_fn(|_| None),
            next_section: 0,
        }
    }
}

impl<S> PathScratch<S> {
    pub fn clear(&mut self) {
        self.block_types.clear();
        self.mob_types.clear();
        self.sections.fill_with(|| None);
        self.next_section = 0;
    }
}
