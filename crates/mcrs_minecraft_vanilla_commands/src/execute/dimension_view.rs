use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_command_engine::CommandFailure;
use mcrs_minecraft_core::heightmap::HeightmapName;
use mcrs_minecraft_core::{BlockPos, BlockStateId, ColumnPos, SectionPos};
use mcrs_minecraft_engine::teleport::MAX_XZ_BLOCKS;
use mcrs_minecraft_heightmap::{
    MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_level::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_level::world::storage::block_entity::{
    BlockEntity, BlockEntityPos, SectionBlockEntities,
};
use mcrs_minecraft_level::world::storage::column::{ColumnIndex, ColumnSections, SectionLookup};

fn local(axis: i32) -> Option<usize> {
    usize::try_from(axis & SectionPos::MASK as i32).ok()
}

pub(super) fn unloaded() -> CommandFailure {
    CommandFailure::translate("argument.pos.unloaded", vec![])
}

/// The blocks, the columns and the heightmaps of the dimension of this world.
#[derive(SystemParam)]
pub(super) struct DimensionView<'w, 's> {
    dimension: Query<'w, 's, (&'static ColumnIndex, &'static DimensionTypeConfig), With<Dimension>>,
    columns: Query<
        'w,
        's,
        (
            &'static ColumnSections,
            Option<&'static SurfaceHeightmap>,
            Option<&'static SolidHeightmap>,
            Option<&'static MotionHeightmap>,
            Option<&'static NoLeavesHeightmap>,
        ),
    >,
    sections: Query<'w, 's, (&'static ChunkBlocks, &'static SectionStage)>,
    held: Query<'w, 's, &'static SectionBlockEntities>,
    block_entities: Query<'w, 's, (&'static BlockEntityPos, &'static BlockEntity)>,
}

impl DimensionView<'_, '_> {
    fn column_entity(&self, x: i32, z: i32) -> Option<Entity> {
        let (index, _) = self.dimension.single().ok()?;
        let slot = index.get(&ColumnPos::new(
            x >> SectionPos::BITS,
            z >> SectionPos::BITS,
        ))?;
        Some(slot.entity)
    }

    fn column(&self, x: i32, z: i32) -> Option<&ColumnSections> {
        let (sections, ..) = self.columns.get(self.column_entity(x, z)?).ok()?;
        Some(sections)
    }

    fn section(&self, [x, y, z]: [i32; 3]) -> Option<Entity> {
        match self.column(x, z)?.lookup(y >> SectionPos::BITS) {
            SectionLookup::Loaded(section) => Some(section),
            SectionLookup::Unloaded | SectionLookup::OutOfRange => None,
        }
    }

    /// The column holds every section of its height, each at the loaded stage.
    // chisle: a loaded chunk and a ticking chunk are the same test: the dimension has no
    // chunk levels for entity ticking yet. A ticking stage of the column lifts it.
    pub(super) fn loaded(&self, x: i32, z: i32) -> bool {
        self.column(x, z).is_some_and(|column| {
            column.iter().all(|section| match section {
                SectionLookup::Loaded(entity) => self
                    .sections
                    .get(entity)
                    .is_ok_and(|(_, stage)| *stage == SectionStage::Loaded),
                SectionLookup::Unloaded | SectionLookup::OutOfRange => false,
            })
        })
    }

    /// CMD-EX-050: the position must be in a loaded chunk and inside the world.
    pub(super) fn readable(&self, [x, y, z]: [i32; 3]) -> Result<(), CommandFailure> {
        if !self.loaded(x, z) {
            return Err(unloaded());
        }
        let (_, config) = self.dimension.single().map_err(|_| unloaded())?;
        let height = i32::try_from(config.height).unwrap_or(i32::MAX);
        let inside = (config.min_y..config.min_y.saturating_add(height)).contains(&y)
            && (-MAX_XZ_BLOCKS..MAX_XZ_BLOCKS).contains(&x)
            && (-MAX_XZ_BLOCKS..MAX_XZ_BLOCKS).contains(&z);
        if !inside {
            return Err(CommandFailure::translate("argument.pos.outofworld", vec![]));
        }
        Ok(())
    }

    pub(super) fn block_state(&self, pos: [i32; 3]) -> Option<BlockStateId> {
        let (blocks, _) = self.sections.get(self.section(pos)?).ok()?;
        let [x, y, z] = pos;
        blocks.get_cell(local(x)?, local(y)?, local(z)?)
    }

    // chisle: a linear scan of the block entities of one section; an index by position in the
    // section lifts it.
    pub(super) fn block_entity(&self, pos: [i32; 3]) -> Option<&GeneratedBlockEntity> {
        let section = self.section(pos)?;
        let [x, y, z] = pos;
        let pos = BlockPos::new(x, y, z);
        self.held.get(section).ok()?.iter().find_map(|entity| {
            let (at, block_entity) = self.block_entities.get(entity).ok()?;
            (at.0 == pos).then_some(&block_entity.0)
        })
    }

    /// HM-006 for a column of a loaded chunk.
    pub(super) fn height(&self, heightmap: HeightmapName, x: i32, z: i32) -> Option<i32> {
        let (_, surface, solid, motion, no_leaves) =
            self.columns.get(self.column_entity(x, z)?).ok()?;
        let heights = match heightmap {
            HeightmapName::WorldSurface | HeightmapName::WorldSurfaceWg => &surface?.0,
            HeightmapName::OceanFloor | HeightmapName::OceanFloorWg => &solid?.0,
            HeightmapName::MotionBlocking => &motion?.0,
            HeightmapName::MotionBlockingNoLeaves => &no_leaves?.0,
        };
        Some(heights.get(local(x)?, local(z)?))
    }
}
