use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashSet;

use crate::CollisionSection;
use crate::CollisionWorld;
use crate::SectionLookup;
use crate::SectionMasks;
use crate::WorldBorder;

/// A base world in which the blocks of `removed` are air: the view of an explosion group, which
/// must not write blocks before the block domain runs.
#[derive(Clone, Copy, Debug)]
pub struct RemovedOverlay<'a, W> {
    pub base: &'a W,
    pub removed: &'a FxHashSet<BlockPos>,
}

/// A section of a [`RemovedOverlay`].
#[derive(Clone, Copy, Debug)]
pub struct OverlaySection<'a, S> {
    base: S,
    origin: BlockPos,
    /// `None` when no removal is in this section.
    removed: Option<&'a FxHashSet<BlockPos>>,
}

impl<W: CollisionWorld> CollisionWorld for RemovedOverlay<'_, W> {
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
                removed: self
                    .removed
                    .iter()
                    .any(|&pos| SectionPos::from(pos) == at)
                    .then_some(self.removed),
            }),
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for OverlaySection<'_, S> {
    /// The masks of the base do not know the removals, so a removal forces the per-block path.
    fn masks(&self) -> Option<&SectionMasks> {
        match self.removed {
            None => self.base.masks(),
            Some(_) => None,
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        let Some(removed) = self.removed else {
            return self.base.state(at);
        };
        let pos = BlockPos::new(
            self.origin.x + i32::from(at.x()),
            self.origin.y + i32::from(at.y()),
            self.origin.z + i32::from(at.z()),
        );
        if removed.contains(&pos) {
            BlockStateId::AIR
        } else {
            self.base.state(at)
        }
    }
}
