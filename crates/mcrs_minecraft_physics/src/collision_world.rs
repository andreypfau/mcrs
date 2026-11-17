use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::SectionMasks;
use crate::WorldBorder;

pub trait CollisionWorld {
    type Section<'a>: CollisionSection
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>>;

    fn border(&self) -> Option<&WorldBorder>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLookup<S> {
    Outside,
    Unloaded,
    Loaded(S),
}

pub trait CollisionSection {
    fn masks(&self) -> Option<&SectionMasks>;

    fn state(&self, at: LocalPos) -> BlockStateId;
}
