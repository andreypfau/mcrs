use std::path::PathBuf;

use bevy_app::{App, AppExit, Plugin, PreStartup};
use bevy_math::DVec3;
use mcrs_minecraft_engine::EnginePlugin;
use mcrs_minecraft_engine::SpawnOverride;
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::coordinator::ops::{DefaultOpLevel, OpList};
use mcrs_minecraft_engine::heightmap::HeightmapPredicatesPlugin;
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_registries::dimension::Dimensions;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_save::{SaveError, SaveLock};
use tracing::error;

use crate::WorldOpenError;
use crate::require::require;
use crate::server_core_plugin::ServerCorePlugin;
use crate::world_options::{
    FailedStart, PluginDimensions, WorldHost, WorldLock, WorldPresetName, bake_dimensions,
};

pub struct DimensionWorldsPlugin {
    pub world: Option<PathBuf>,
    pub column_traces: Option<ColumnTraceSink>,
    pub seed: u64,
    pub preset: String,
    pub dimensions: Option<Dimensions>,
    pub ops: OpList,
    pub default_op_level: u8,
    pub default_game_mode: GameMode,
    pub host: Option<Uuid>,
    pub spawn: Option<[f64; 3]>,
}

impl Default for DimensionWorldsPlugin {
    fn default() -> Self {
        Self {
            world: None,
            column_traces: None,
            seed: 0,
            preset: "normal".to_owned(),
            dimensions: None,
            ops: OpList::default(),
            default_op_level: 0,
            default_game_mode: GameMode::Creative,
            host: None,
            spawn: None,
        }
    }
}

impl Plugin for DimensionWorldsPlugin {
    fn build(&self, app: &mut App) {
        require::<Self, ServerCorePlugin>(app);
        app.insert_resource(WorldSeed(self.seed));
        app.insert_resource(WorldPresetName(self.preset.clone()));
        if let Some(world) = &self.world {
            let handed_in = app
                .world()
                .get_resource::<WorldLock>()
                .is_some_and(|lock| lock.0.dir() == world);
            let locked = handed_in
                || match SaveLock::take(world) {
                    Ok(lock) => {
                        app.insert_resource(WorldLock(lock));
                        true
                    }
                    Err(error) => {
                        error!("the world does not open:\n{error}");
                        let error = match error {
                            SaveError::Locked { dir } => WorldOpenError::Locked { dir },
                            error => WorldOpenError::Save(error),
                        };
                        app.insert_resource(FailedStart(error));
                        app.world_mut().write_message(AppExit::error());
                        false
                    }
                };
            if locked {
                app.insert_resource(WorldSave(world.clone()));
            }
        }
        if let Some(host) = self.host {
            app.insert_resource(WorldHost(host));
        }
        if let Some(dimensions) = &self.dimensions {
            app.insert_resource(PluginDimensions(dimensions.clone()));
        }
        app.insert_resource(self.ops.clone());
        app.insert_resource(DefaultOpLevel(PlayerOpLevel(self.default_op_level)));
        app.insert_resource(DefaultGameMode(self.default_game_mode));
        if let Some(spawn) = self.spawn.map(DVec3::from_array) {
            app.insert_resource(SpawnOverride(spawn));
        }
        if let Some(traces) = &self.column_traces {
            app.insert_resource(traces.clone());
        }
        app.add_systems(PreStartup, bake_dimensions);
        app.add_plugins(HeightmapPredicatesPlugin);
        app.add_plugins(EnginePlugin);
    }
}
