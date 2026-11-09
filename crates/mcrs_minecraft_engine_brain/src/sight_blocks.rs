use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_minecraft_volume::SectionBlocks;
use mcrs_minecraft_volume::Unavailable;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

/// The blocks of a volume as the sight ray reads them: each section without masks, since the ray
/// reads each block that it crosses, and a section of another task as one that is not loaded.
pub(crate) struct SightBlocks<'a, V>(pub(crate) &'a V);

/// A loaded section of [`SightBlocks`].
pub(crate) struct SightSection<S>(S);

impl<V: BlockVolume> CollisionWorld for SightBlocks<'_, V> {
    type Section<'s>
        = SightSection<V::Section<'s>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.0.section(at) {
            Ok(blocks) => SectionLookup::Loaded(SightSection(blocks)),
            Err(Unavailable::Outside) => SectionLookup::Outside,
            Err(Unavailable::Unloaded | Unavailable::Foreign) => SectionLookup::Unloaded,
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        None
    }
}

impl<S: SectionBlocks> CollisionSection for SightSection<S> {
    fn masks(&self) -> Option<&SectionMasks> {
        None
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.0.get(at)
    }
}
