use bevy_app::SubApp;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::world::dimension::DimensionTypeConfig;

pub struct DimensionBuild<'a> {
    pub dimension: &'a ResourceKeyBuf<Dimension>,
    pub type_config: DimensionTypeConfig,
    pub has_sky_light: bool,
}

pub type DimensionBuilder = fn(&World, &DimensionBuild<'_>, &mut SubApp);

pub type DimensionHostBuilder = fn(&mut World, Entity, &DimensionBuild<'_>, &World);

#[derive(Resource, Default)]
pub struct DimensionBuilders {
    cells: Vec<DimensionBuilder>,
    hosts: Vec<DimensionHostBuilder>,
}

impl DimensionBuilders {
    pub fn add(&mut self, builder: DimensionBuilder) {
        self.cells.push(builder);
    }

    pub fn add_host(&mut self, builder: DimensionHostBuilder) {
        self.hosts.push(builder);
    }

    pub fn build(&self, host: &World, dimension: &DimensionBuild<'_>, sub_app: &mut SubApp) {
        for builder in &self.cells {
            builder(host, dimension, sub_app);
        }
    }

    #[must_use]
    pub fn hosts(&self) -> &[DimensionHostBuilder] {
        &self.hosts
    }
}
