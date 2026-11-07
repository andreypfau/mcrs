use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionTable;
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

/// The blocks of a volume as the kernel reads them. A section of one state carries its masks; any
/// other section takes the per-block path.
pub(crate) struct BlockWorld<'a, V> {
    pub(crate) blocks: &'a V,
    pub(crate) table: &'a CollisionTable,
}

/// A loaded section of a [`BlockWorld`].
pub(crate) struct BlockSection<S> {
    blocks: S,
    masks: Option<SectionMasks>,
}

impl<V: BlockVolume> CollisionWorld for BlockWorld<'_, V> {
    type Section<'s>
        = BlockSection<V::Section<'s>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.blocks.section(at) {
            Ok(blocks) => {
                let masks = blocks.uniform().map(|state| self.uniform_masks(state));
                SectionLookup::Loaded(BlockSection { blocks, masks })
            }
            Err(Unavailable::Outside) => SectionLookup::Outside,
            Err(Unavailable::Unloaded | Unavailable::Foreign) => SectionLookup::Unloaded,
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        None
    }
}

impl<V> BlockWorld<'_, V> {
    fn uniform_masks(&self, state: BlockStateId) -> SectionMasks {
        let solid = self.table.state(state).is_solid();
        SectionMasks {
            solid: if solid { u64::MAX } else { 0 },
            air_only: state == BlockStateId::AIR,
        }
    }
}

impl<S: SectionBlocks> CollisionSection for BlockSection<S> {
    fn masks(&self) -> Option<&SectionMasks> {
        self.masks.as_ref()
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        self.blocks.get(at)
    }
}
