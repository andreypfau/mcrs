use bevy_app::SubApp;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::world::dimension::DimensionTypeConfig;

/// The dimension a world is built for.
pub struct DimensionBuild<'a> {
    pub dimension: &'a ResourceKeyBuf<Dimension>,
    pub type_config: DimensionTypeConfig,
    pub has_sky_light: bool,
}

/// Reads the host world and adds a host plugin's part to one dimension world.
pub type DimensionBuilder = fn(&World, &DimensionBuild<'_>, &mut SubApp);

/// Adds a host plugin's part for one dimension to the host world, once for all the cells of the
/// dimension: the host world, the label entity of the dimension, the dimension, and the world of
/// its first cell after the [`DimensionBuilder`]s ran.
pub type DimensionHostBuilder = fn(&mut World, Entity, &DimensionBuild<'_>, &World);

/// The builders every dimension world runs in order, before
/// [`DimensionPlugins`](crate::world::sub_app::DimensionPlugins).
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
