use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;

/// The world as one candidate of the client sees it: the current world with the block changes
/// that the candidate has not applied yet undone to their old states.
#[derive(Clone, Copy, Debug)]
pub struct ClientView<'a, W> {
    pub base: &'a W,
    pub undone: &'a FxHashMap<BlockPos, BlockStateId>,
}

/// A section of a [`ClientView`].
#[derive(Clone, Copy, Debug)]
pub struct ClientSection<'a, S> {
    base: S,
    origin: BlockPos,
    /// `None` when no undone block is in this section.
    undone: Option<&'a FxHashMap<BlockPos, BlockStateId>>,
}

impl<W: CollisionWorld> CollisionWorld for ClientView<'_, W> {
    type Section<'a>
        = ClientSection<'a, W::Section<'a>>
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.base.section(at) {
            SectionLookup::Outside => SectionLookup::Outside,
            SectionLookup::Unloaded => SectionLookup::Unloaded,
            SectionLookup::Loaded(base) => SectionLookup::Loaded(ClientSection {
                base,
                origin: at.min_block(),
                undone: (!self.undone.is_empty()
                    && self.undone.keys().any(|&pos| SectionPos::from(pos) == at))
                .then_some(self.undone),
            }),
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for ClientSection<'_, S> {
    /// The masks of the base do not know the undone blocks, so one forces the per-block path.
    fn masks(&self) -> Option<&SectionMasks> {
        match self.undone {
            None => self.base.masks(),
            Some(_) => None,
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        let Some(undone) = self.undone else {
            return self.base.state(at);
        };
        let pos = BlockPos::new(
            self.origin.x + i32::from(at.x()),
            self.origin.y + i32::from(at.y()),
            self.origin.z + i32::from(at.z()),
        );
        undone
            .get(&pos)
            .copied()
            .unwrap_or_else(|| self.base.state(at))
    }
}
