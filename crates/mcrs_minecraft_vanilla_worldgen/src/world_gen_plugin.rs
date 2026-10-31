use std::time::Duration;

use bevy_app::{App, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_level::world::dimension_builders::DimensionBuilders;
use mcrs_minecraft_level::world::sub_app::DimSpawnEnqueue;

use crate::chunk_plugin::SlowColumnThreshold;
use crate::dimension_world::build_dimension_world;
use crate::features::FeaturePlugin;
use crate::modern_carvers::ModernCarverPlugin;
use crate::routers::build_dimension_routers;
use crate::structures::{StructurePlugin, build_dimension_structures};

/// Generates and reads the columns of every dimension. The host inserts the
/// [`DimensionList`](mcrs_minecraft_level::world::dimension_list::DimensionList),
/// [`DimensionGenerators`](mcrs_minecraft_engine::dimension_generators::DimensionGenerators) and [`WorldSeed`](mcrs_minecraft_engine::world_seed::WorldSeed) before
/// `OnEnter(AppState::Playing)`.
#[derive(Clone, Debug)]
pub struct WorldGenPlugin {
    /// A column that takes longer than this from request to delivery is logged.
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
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(build_dimension_world);
    }
}
