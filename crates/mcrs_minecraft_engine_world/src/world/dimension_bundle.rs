use crate::world::dimension::{Dimension, DimensionTypeConfig};
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::simulation::SectionSimulation;
use crate::world::storage::column::ColumnIndex;
use crate::world::storage::section::SectionIndex;
use bevy_ecs::prelude::Bundle;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_voxel::SectionPos;

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
