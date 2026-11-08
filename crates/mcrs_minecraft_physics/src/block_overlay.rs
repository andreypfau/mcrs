use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;

use crate::CollisionSection;
use crate::CollisionWorld;
use crate::SectionLookup;
use crate::SectionMasks;
use crate::WorldBorder;

/// A base world in which the blocks of `blocks` have the given states, without a write to the
/// base: an explosion group sees its removed blocks as air before the block domain runs, and a
/// client sees the block changes that it has not applied yet as their old states.
#[derive(Clone, Copy, Debug)]
pub struct BlockOverlay<'a, W> {
    pub base: &'a W,
    pub blocks: &'a FxHashMap<BlockPos, BlockStateId>,
}

/// A section of a [`BlockOverlay`].
#[derive(Clone, Copy, Debug)]
pub struct OverlaySection<'a, S> {
    base: S,
    origin: BlockPos,
    /// `None` when no overlay block is in this section.
    blocks: Option<&'a FxHashMap<BlockPos, BlockStateId>>,
}

impl<W: CollisionWorld> CollisionWorld for BlockOverlay<'_, W> {
    type Section<'a>
        = OverlaySection<'a, W::Section<'a>>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.base.section(at) {
            SectionLookup::Outside => SectionLookup::Outside,
            SectionLookup::Unloaded => SectionLookup::Unloaded,
            SectionLookup::Loaded(base) => SectionLookup::Loaded(OverlaySection {
                base,
                origin: at.min_block(),
                blocks: self
                    .blocks
                    .keys()
                    .any(|&pos| SectionPos::from(pos) == at)
                    .then_some(self.blocks),
            }),
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for OverlaySection<'_, S> {
    /// The masks of the base do not know the overlay, so an overlay block forces the per-block
    /// path.
    fn masks(&self) -> Option<&SectionMasks> {
        match self.blocks {
            None => self.base.masks(),
            Some(_) => None,
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        let Some(blocks) = self.blocks else {
            return self.base.state(at);
        };
        let pos = BlockPos::new(
            self.origin.x + i32::from(at.x()),
            self.origin.y + i32::from(at.y()),
            self.origin.z + i32::from(at.z()),
        );
        blocks
            .get(&pos)
            .copied()
            .unwrap_or_else(|| self.base.state(at))
    }
}
