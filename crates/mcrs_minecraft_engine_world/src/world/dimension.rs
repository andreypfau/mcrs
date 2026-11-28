use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{Component, ContainsEntity, Entity};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::SectionPos;

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
