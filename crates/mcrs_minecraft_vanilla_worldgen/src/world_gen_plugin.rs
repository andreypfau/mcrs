use std::time::Duration;

use bevy_app::{App, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::dim_tick::{TickDimensions, TickDimensionsSet};
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuilders;
use mcrs_minecraft_engine_world::world::sub_app::DimSpawnEnqueue;

use crate::chunk_plugin::SlowColumnThreshold;
use crate::dimension_world::build_dimension_world;
use crate::features::FeaturePlugin;
use crate::modern_carvers::ModernCarverPlugin;
use crate::routers::build_dimension_routers;
use crate::structures::{StructurePlugin, build_dimension_structures};
use crate::world_generation::{
    WorldGeneration, ask_world_columns, build_world_generation, deliver_world_columns,
};

/// Generates and reads the columns of every dimension. The host inserts the
/// [`DimensionList`](mcrs_minecraft_engine_world::world::dimension_list::DimensionList),
/// [`DimensionGenerators`](mcrs_minecraft_engine::dimension_generators::DimensionGenerators) and [`WorldSeed`](mcrs_minecraft_engine::world_seed::WorldSeed) before
/// `OnEnter(AppState::Playing)`.
#[derive(Clone, Debug)]
pub struct WorldGenPlugin {
    /// A column that takes longer than this from its request to its landing is logged.
    pub slow_column_threshold: Duration,
}

impl Default for WorldGenPlugin {
    fn default() -> Self {
        Self {
            slow_column_threshold: SlowColumnThreshold::default().0,
        }
    }
}

impl Plugin for WorldGenPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SlowColumnThreshold(self.slow_column_threshold));
        app.add_plugins((ModernCarverPlugin, FeaturePlugin, StructurePlugin));
        app.add_systems(
            OnEnter(AppState::Playing),
            build_dimension_routers
                .after(build_dimension_structures)
                .before(DimSpawnEnqueue),
        );
        let mut builders = app.world_mut().get_resource_or_init::<DimensionBuilders>();
        builders.add(build_dimension_world);
        builders.add_host(build_world_generation);
        app.init_resource::<WorldGeneration>();
        app.add_systems(
            TickDimensions,
            (
                deliver_world_columns.before(TickDimensionsSet::Tick),
                ask_world_columns.after(TickDimensionsSet::Tick),
            ),
        );
    }
}
