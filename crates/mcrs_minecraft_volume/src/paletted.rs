use mcrs_minecraft_chunk::PalettedContainer;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::{LocalPos, SectionPos};

use crate::block::{Palette, PaletteBuffer, SectionBlocks, SectionBlocksMut};

pub(crate) type Container = PalettedContainer<BlockStateId, { SectionPos::SIZE }>;

/// A section kept as a paletted container: its palette is read, never rebuilt.
#[derive(Clone, Copy, Debug)]
pub struct PalettedSection<'a>(pub &'a Container);

#[derive(Debug)]
pub struct PalettedSectionMut<'a>(pub &'a mut Container);

fn palette_of(container: &Container) -> Palette<'_> {
    match container {
        PalettedContainer::Homogeneous(value) => Palette::uniform(*value),
        PalettedContainer::Heterogeneous(data) => Palette::listed(&data.palette, &data.counts),
    }
}

impl SectionBlocks for PalettedSection<'_> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        self.0.get_local(at)
    }

    fn uniform(&self) -> Option<BlockStateId> {
        match self.0 {
            PalettedContainer::Homogeneous(value) => Some(*value),
            PalettedContainer::Heterogeneous(_) => None,
        }
    }

    fn palette<'s>(&'s self, _: &'s mut PaletteBuffer) -> Palette<'s> {
        palette_of(self.0)
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(LocalPos, BlockStateId),
    {
        let mut index = 0usize;
        self.0.for_each(|value| {
            f(LocalPos::from_index(index), value);
            index = index.wrapping_add(1);
        });
    }
}

impl PalettedSectionMut<'_> {
    fn shared(&self) -> PalettedSection<'_> {
        PalettedSection(self.0)
    }
}

impl SectionBlocks for PalettedSectionMut<'_> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        self.shared().get(at)
    }

    fn uniform(&self) -> Option<BlockStateId> {
        self.shared().uniform()
    }

    fn palette<'s>(&'s self, _: &'s mut PaletteBuffer) -> Palette<'s> {
        palette_of(self.0)
    }

    fn for_each<F>(&self, f: F)
    where
        F: FnMut(LocalPos, BlockStateId),
    {
        self.shared().for_each(f);
    }
}

impl SectionBlocksMut for PalettedSectionMut<'_> {
    #[inline]
    fn set(&mut self, at: LocalPos, state: BlockStateId) -> BlockStateId {
        self.0.set_local(at, state)
    }

    fn fill(&mut self, state: BlockStateId) {
        *self.0 = PalettedContainer::Homogeneous(state);
    }

    fn fill_box(&mut self, min: LocalPos, max: LocalPos, state: BlockStateId) {
        let end = |at: u8| usize::from(at).wrapping_add(1);
        self.0.fill_box(
            usize::from(min.x()),
            end(max.x()),
            usize::from(min.y()),
            end(max.y()),
            usize::from(min.z()),
            end(max.z()),
            state,
        );
    }
}
