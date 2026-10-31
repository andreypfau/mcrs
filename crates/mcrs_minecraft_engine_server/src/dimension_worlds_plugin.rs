use std::path::PathBuf;

use bevy_app::{App, Plugin, Startup};
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::heightmap::HeightmapPredicatesPlugin;
use mcrs_minecraft_engine::ops::{DefaultOpLevel, OpList};
use mcrs_minecraft_engine::world_plugin::WorldPlugin;
use mcrs_minecraft_engine::world_seed::WorldSeed;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_level::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_level::world::world_save::WorldSave;
use mcrs_minecraft_world::dimension::Dimensions;

use crate::require::require;
use crate::server_core_plugin::ServerCorePlugin;
use crate::world_options::{PluginDimensions, WorldPresetName, bake_dimensions};

/// The dimension list and one world per dimension, with the bridge that carries each
/// player's packets between the network and the player's dimension.
pub struct DimensionWorldsPlugin {
    /// World folder to read saved chunks from. Without one, and for any column
    /// the folder has never saved, the dimension generates its terrain.
    pub world: Option<PathBuf>,
    /// Shared with a client in the same process, whose debug views read each
    /// dimension's column lifecycle from it.
    pub column_traces: Option<ColumnTraceSink>,
    /// The seed of a world without a save; a save's own seed replaces it.
    pub seed: u64,
    /// The world preset that names the dimensions when neither the save nor `dimensions` does.
    pub preset: String,
    /// The dimensions the world spawns, in place of a save's list and the world preset; the
    /// overworld is then optional, and a player with no saved dimension spawns in the first.
    pub dimensions: Option<Dimensions>,
    pub ops: OpList,
    /// Operator level of a player who has no entry in `ops`.
    pub default_op_level: u8,
    /// Game mode of a player joining for the first time.
    pub default_game_mode: GameMode,
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
        }
    }
}

impl Plugin for DimensionWorldsPlugin {
    fn build(&self, app: &mut App) {
        require::<Self, ServerCorePlugin>(app);
        app.insert_resource(WorldSeed(self.seed));
        app.insert_resource(WorldPresetName(self.preset.clone()));
        if let Some(world) = &self.world {
            app.insert_resource(WorldSave(world.clone()));
        }
        if let Some(dimensions) = &self.dimensions {
            app.insert_resource(PluginDimensions(dimensions.clone()));
        }
        app.insert_resource(self.ops.clone());
        app.insert_resource(DefaultOpLevel(PlayerOpLevel(self.default_op_level)));
        app.insert_resource(DefaultGameMode(self.default_game_mode));
        if let Some(traces) = &self.column_traces {
            app.insert_resource(traces.clone());
        }
        app.add_systems(Startup, bake_dimensions);
        app.add_plugins(HeightmapPredicatesPlugin);
        app.add_plugins(WorldPlugin);
    }
}
