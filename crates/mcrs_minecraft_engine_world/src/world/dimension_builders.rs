use bevy_app::SubApp;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;

use crate::world::dimension::DimensionTypeConfig;

/// The dimension a world is built for.
pub struct DimensionBuild<'a> {
    pub dimension: &'a ResourceKey<Dimension>,
    pub type_config: DimensionTypeConfig,
    pub has_sky_light: bool,
}

/// Reads the host world and adds a host plugin's part to one dimension world.
pub type DimensionBuilder = fn(&World, &DimensionBuild<'_>, &mut SubApp);

/// The builders every dimension world runs in order, before
/// [`DimensionPlugins`](crate::world::sub_app::DimensionPlugins).
#[derive(Resource, Default)]
pub struct DimensionBuilders(Vec<DimensionBuilder>);

impl DimensionBuilders {
    pub fn add(&mut self, builder: DimensionBuilder) {
        self.0.push(builder);
    }

    pub fn build(&self, host: &World, dimension: &DimensionBuild<'_>, sub_app: &mut SubApp) {
        for builder in &self.0 {
            builder(host, dimension, sub_app);
        }
    }
}
