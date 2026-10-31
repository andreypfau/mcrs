use std::sync::Arc;

use bevy_app::{App, Plugin, Startup, SubApp};
use bevy_ecs::prelude::{Commands, Res};
use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_level::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_light::block_light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_light::level::LightBounds;

use crate::dim_light_plugin::DimLightPlugin;

/// Light propagation in every dimension. Without it every column goes to the client at full
/// sky light, and the world loads without the propagation cost.
///
/// The host inserts [`Blocks`] before `Startup`.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, insert_block_light_registry);
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(build_dimension_light);
    }
}

fn insert_block_light_registry(mut commands: Commands, blocks: Res<Blocks>) {
    commands.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}

fn build_dimension_light(host: &World, build: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    let Some(registry) = host.get_resource::<BlockLightRegistry>() else {
        return;
    };
    sub_app.add_plugins(DimLightPlugin {
        registry: Arc::clone(&registry.0),
        bounds: LightBounds::from_dimension(
            build.type_config.min_y,
            build.type_config.section_count,
        ),
        sky: build.has_sky_light,
    });
}
