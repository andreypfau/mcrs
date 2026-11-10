use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_command_engine::CommandFailure;
use mcrs_minecraft_engine::teleport::MAX_XZ_BLOCKS;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::column::{
    ColumnIndex, ColumnSections, SectionLookup,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

/// The blocks of the dimension of this world, as a command reads them.
#[derive(SystemParam)]
pub(crate) struct BlockView<'w, 's> {
    dimension: Query<
        'w,
        's,
        (
            Entity,
            &'static ColumnIndex,
            &'static DimensionTypeConfig,
            &'static SectionIndex,
        ),
        With<Dimension>,
    >,
    columns: Query<'w, 's, &'static ColumnSections>,
    stages: Query<'w, 's, &'static SectionStage>,
}

impl BlockView<'_, '_> {
    /// The dimension entity of this world.
    pub(crate) fn dimension(&self) -> Option<Entity> {
        self.dimension.single().ok().map(|(entity, ..)| entity)
    }

    fn column(&self, column: ColumnPos) -> Option<&ColumnSections> {
        let (_, index, ..) = self.dimension.single().ok()?;
        self.columns.get(index.get(&column)?.entity).ok()
    }

    /// Every section of the column of `pos` is loaded.
    pub(crate) fn is_loaded(&self, pos: BlockPos) -> bool {
        self.loaded(pos.x, pos.z)
    }

    fn loaded(&self, x: i32, z: i32) -> bool {
        let column = ColumnPos::new(x >> SectionPos::BITS, z >> SectionPos::BITS);
        self.column(column).is_some_and(|sections| {
            sections.iter().all(|section| match section {
                SectionLookup::Loaded(entity) => self
                    .stages
                    .get(entity)
                    .is_ok_and(|stage| *stage == SectionStage::Loaded),
                SectionLookup::Unloaded | SectionLookup::OutOfRange => false,
            })
        })
    }

    /// The position is in a loaded column and inside the world.
    pub(crate) fn readable(&self, pos: BlockPos) -> Result<(), CommandFailure> {
        if !self.loaded(pos.x, pos.z) {
            return Err(CommandFailure::translate("argument.pos.unloaded", vec![]));
        }
        let unloaded = || CommandFailure::translate("argument.pos.unloaded", vec![]);
        let (_, _, config, _) = self.dimension.single().map_err(|_| unloaded())?;
        let height = i32::try_from(config.height).unwrap_or(i32::MAX);
        let inside = (config.min_y..config.min_y.saturating_add(height)).contains(&pos.y)
            && (-MAX_XZ_BLOCKS..MAX_XZ_BLOCKS).contains(&pos.x)
            && (-MAX_XZ_BLOCKS..MAX_XZ_BLOCKS).contains(&pos.z);
        if !inside {
            return Err(CommandFailure::translate("argument.pos.outofworld", vec![]));
        }
        Ok(())
    }

    /// The state at `pos`, or `None` when its section is not loaded.
    pub(crate) fn block_state(&self, pos: BlockPos) -> Option<BlockStateId> {
        let (.., sections) = self.dimension.single().ok()?;
        let local = |axis: i32| usize::try_from(axis & SectionPos::MASK as i32).ok();
        sections.blocks(SectionPos::from(pos))?.get_cell(
            local(pos.x)?,
            local(pos.y)?,
            local(pos.z)?,
        )
    }
}
