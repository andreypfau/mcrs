use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::SectionMasks;
use crate::WorldBorder;

/// The block states, loaded sections and world border that a step reads. The host implements
/// it for its own storage.
pub trait CollisionWorld {
    type Section<'a>: CollisionSection
    where
        Self: 'a;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>>;

    fn border(&self) -> Option<&WorldBorder>;
}

/// A section that a [`CollisionWorld`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionLookup<S> {
    /// Out of the world: no block and no collider.
    Outside,
    /// In a chunk that is not loaded: no collider.
    Unloaded,
    Loaded(S),
}

/// One loaded section.
pub trait CollisionSection {
    /// `None` when the section has an overlay: the caller takes the per-block path.
    fn masks(&self) -> Option<&SectionMasks>;

    fn state(&self, at: LocalPos) -> BlockStateId;
}
