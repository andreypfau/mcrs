use crate::tick::TickSet;
use crate::world::dimension::{Dimension, DimensionTypeConfig};
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::simulation::SectionSimulation;
use crate::world::lifecycle::ticket::TicketPlugin;
use crate::world::storage::column::{ColumnIndex, ColumnPlugin};
use crate::world::storage::section::SectionIndex;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{Bundle, IntoScheduleConfigs};
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::SectionPos;

pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                TickSet::DomainInput,
                TickSet::ClientInput,
                TickSet::Simulate,
                TickSet::WorldWrite,
                TickSet::Project,
                TickSet::Output,
            )
                .chain(),
        );
        app.add_plugins(TicketPlugin);
        app.add_plugins(ColumnPlugin);
    }
}

#[derive(Bundle)]
pub struct DimensionBundle {
    pub dimension: Dimension,
    pub key: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
    pub type_config: DimensionTypeConfig,
    pub chunk_index: SectionIndex,
    pub loading: ColumnLoading,
    pub simulation: SectionSimulation,
    pub column_index: ColumnIndex,
}

impl DimensionBundle {
    pub fn new(
        key: ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>,
        type_config: DimensionTypeConfig,
    ) -> Self {
        let min_section_y = type_config.min_y >> SectionPos::BITS;
        let max_section_y = min_section_y + type_config.section_count as i32 - 1;
        Self {
            dimension: Dimension,
            key,
            type_config,
            chunk_index: SectionIndex::default(),
            loading: ColumnLoading::bounded(min_section_y..=max_section_y),
            simulation: SectionSimulation::default(),
            column_index: ColumnIndex::default(),
        }
    }
}
