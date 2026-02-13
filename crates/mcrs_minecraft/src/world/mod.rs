use crate::configuration::{LoadedDimensionTypes, LoadedWorldPreset};
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::BlockUpdatePlugin;
use crate::world::chunk::ChunkPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use bevy_app::{App, Plugin, PreStartup};
use bevy_ecs::prelude::*;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionPlugin, DimensionTypeConfig,
};
use mcrs_protocol::WritePacket;

pub mod block;
mod block_update;
pub mod chunk;
pub mod entity;
mod explosion;
mod format;
mod generate;
mod inventory;
pub mod item;
mod material;
mod palette;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DimensionPlugin);
        app.add_systems(PreStartup, spawn_dimensions_from_preset);
        app.add_plugins(ChunkPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_plugins(ExplosionPlugin);
    }
}

/// Spawns Dimension entities from the LoadedWorldPreset resource.
/// Each dimension in the preset gets a corresponding ECS entity with DimensionBundle.
fn spawn_dimensions_from_preset(
    mut commands: Commands,
    world_preset: Res<LoadedWorldPreset>,
    dimension_types: Res<LoadedDimensionTypes>,
) {
    if world_preset.dimensions.is_empty() {
        // Fallback: spawn a default overworld dimension if preset is empty
        eprintln!(
            "Warning: LoadedWorldPreset has no dimensions, spawning default overworld dimension"
        );
        commands.spawn(DimensionBundle::default());
        return;
    }

    println!(
        "Spawning {} dimensions from preset '{}'",
        world_preset.dimensions.len(),
        world_preset.preset_name
    );

    for (dimension_key, dimension_type_ref) in &world_preset.dimensions {
        // Find the dimension type configuration by matching the type reference
        let type_config = dimension_types
            .0
            .iter()
            .find(|(id, _)| id.as_str() == dimension_type_ref.as_str())
            .map(|(_, dim_type)| DimensionTypeConfig::new(dim_type.min_y, dim_type.height))
            .unwrap_or_else(|| {
                eprintln!(
                    "Warning: Dimension type '{}' not found for dimension '{}', using default config",
                    dimension_type_ref.as_str(),
                    dimension_key.as_str()
                );
                DimensionTypeConfig::default()
            });

        println!(
            "  Spawning dimension '{}' (type: '{}') with min_y={}, height={}, sections={}",
            dimension_key.as_str(),
            dimension_type_ref.as_str(),
            type_config.min_y,
            type_config.height,
            type_config.section_count
        );

        commands.spawn(DimensionBundle {
            dimension_id: DimensionId::new(dimension_key.as_str()),
            type_config,
            ..Default::default()
        });
    }

    println!(
        "Spawned {} dimension entities from world preset",
        world_preset.dimensions.len()
    );
}
