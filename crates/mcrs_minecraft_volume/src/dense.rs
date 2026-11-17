use mcrs_voxel::{BlockStateId, LocalPos, SectionPos};

use crate::block::SectionBlocks;

static AIR: [BlockStateId; SectionPos::VOLUME] = [BlockStateId::AIR; SectionPos::VOLUME];

#[derive(Clone, Copy, Debug)]
pub struct DenseSection<'a>(pub &'a [BlockStateId; SectionPos::VOLUME]);

impl DenseSection<'static> {
    pub fn air() -> Self {
        Self(&AIR)
    }
}

impl SectionBlocks for DenseSection<'_> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        self.0.get(at.index()).copied().unwrap_or(BlockStateId::AIR)
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(LocalPos, BlockStateId),
    {
        for (index, state) in self.0.iter().enumerate() {
            f(LocalPos::from_index(index), *state);
        }
    }

    fn copy_to(&self, out: &mut [BlockStateId; SectionPos::VOLUME]) {
        out.copy_from_slice(self.0);
    }
}
