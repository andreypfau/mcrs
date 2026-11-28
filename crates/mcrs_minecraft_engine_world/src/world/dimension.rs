use crate::tick::TickSet;
use crate::world::lifecycle::column_loading::ColumnLoading;
use crate::world::lifecycle::simulation::SectionSimulation;
use crate::world::storage::column::ColumnIndex;
use crate::world::storage::section::SectionIndex;
use crate::world::storage::section::SectionPlugin;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{Bundle, Component, ContainsEntity, Entity, IntoScheduleConfigs};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};
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
        app.add_plugins(SectionPlugin);
        app.add_plugins(crate::world::storage::column::ColumnPlugin);
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

#[derive(Component, Default)]
pub struct Dimension;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HasSkyLight;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HasWeather;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimensionTypeId(pub Id<DimensionType>);

#[derive(Component, Clone, Debug, Copy, PartialEq, Eq, DerefMut, Deref)]
pub struct InDimension(pub Entity);

impl ContainsEntity for InDimension {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct DimensionTypeConfig {
    pub min_y: i32,
    pub height: u32,
    pub section_count: u32,
}

impl DimensionTypeConfig {
    pub fn new(min_y: i32, height: u32) -> Self {
        Self {
            min_y,
            height,
            section_count: height >> SectionPos::BITS,
        }
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.height as i32 - 1
    }
}
