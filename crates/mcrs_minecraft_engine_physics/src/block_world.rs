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

#[cfg(test)]
mod tests {
    use mcrs_minecraft_physics::TestBlocks;
    use mcrs_minecraft_volume::BlockVolumeMut;
    use mcrs_minecraft_volume::SparseVolume;
    use mcrs_voxel::BlockPos;

    use super::*;

    #[test]
    fn the_masks_of_a_uniform_section_equal_the_masks_from_its_blocks() {
        let blocks = TestBlocks::new();
        let mut volume = SparseVolume::unbounded();
        for (y, state) in [
            (0, BlockStateId::AIR),
            (1, blocks.stone),
            (2, BlockStateId::AIR),
        ] {
            volume.insert(SectionPos::new(0, y, 0), state);
        }
        volume
            .set_block(BlockPos::new(3, 40, 5), blocks.stone)
            .unwrap();
        let world = BlockWorld {
            blocks: &volume,
            table: &blocks.table,
        };
        for y in 0..3 {
            let SectionLookup::Loaded(section) = world.section(SectionPos::new(0, y, 0)) else {
                panic!("section {y} is loaded");
            };
            let expected =
                (y < 2).then(|| SectionMasks::from_states(&blocks.table, |at| section.state(at)));
            assert_eq!(section.masks(), expected.as_ref(), "section {y}");
        }
    }
}
