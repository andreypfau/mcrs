use crate::configuration::{LoadedDimensionTypes, LoadedWorldPreset};
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::BlockUpdatePlugin;
use crate::world::chunk::ChunkPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use crate::world::loot::LootPlugin;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_engine::world::dimension::{
    Dimension, DimensionBundle, DimensionId, DimensionPlugin, DimensionTypeConfig,
};
use tracing::{debug, info, warn};

pub mod block;
mod block_update;
pub mod chunk;
pub mod entity;
mod explosion;
mod format;
mod generate;
mod inventory;
pub mod item;
pub mod loot;
mod material;
mod palette;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DimensionPlugin);
        // Spawn dimensions in Update after world preset is loaded via Bevy assets
        app.add_systems(Update, spawn_dimensions_from_preset);
        app.add_plugins(ChunkPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_plugins(ExplosionPlugin);
        app.add_plugins(LootPlugin);
    }
}

/// Spawns Dimension entities from the LoadedWorldPreset resource.
/// Each dimension in the preset gets a corresponding ECS entity with DimensionBundle.
///
/// This system runs in Update and waits for the world preset to be loaded via Bevy assets.
/// It only spawns dimensions once, checking if any Dimension entities already exist.
fn spawn_dimensions_from_preset(
    mut commands: Commands,
    world_preset: Res<LoadedWorldPreset>,
    dimension_types: Res<LoadedDimensionTypes>,
    existing_dimensions: Query<Entity, With<Dimension>>,
) {
    // Only spawn dimensions if:
    // 1. The world preset is loaded
    // 2. No dimensions exist yet
    if !world_preset.is_loaded {
        return;
    }

    if !existing_dimensions.is_empty() {
        // Dimensions already spawned
        return;
    }

    if world_preset.dimensions.is_empty() {
        // Fallback: spawn a default overworld dimension if preset is empty
        warn!(
            "LoadedWorldPreset has no dimensions, spawning default overworld dimension"
        );
        commands.spawn(DimensionBundle::default());
        return;
    }

    debug!(
        preset = %world_preset.preset_name,
        dimension_count = world_preset.dimensions.len(),
        "Spawning dimensions from loaded world preset"
    );

    for (dimension_key, dimension_type_ref) in &world_preset.dimensions {
        // Find the dimension type configuration by matching the type reference
        let type_config = dimension_types
            .0
            .iter()
            .find(|(id, _)| id.as_str() == dimension_type_ref.as_str())
            .map(|(_, dim_type)| DimensionTypeConfig::new(dim_type.min_y, dim_type.height))
            .unwrap_or_else(|| {
                warn!(
                    dimension_type = %dimension_type_ref,
                    dimension_key = %dimension_key,
                    "Dimension type not found, using default config"
                );
                DimensionTypeConfig::default()
            });

        info!(
            dimension_key = %dimension_key,
            dimension_type = %dimension_type_ref,
            min_y = type_config.min_y,
            height = type_config.height,
            sections = type_config.section_count,
            "Spawning dimension"
        );

        commands.spawn(DimensionBundle {
            dimension_id: DimensionId::new(dimension_key.as_str()),
            type_config,
            ..Default::default()
        });
    }

    info!(
        dimension_count = world_preset.dimensions.len(),
        "All dimensions spawned from world preset"
    );
}
