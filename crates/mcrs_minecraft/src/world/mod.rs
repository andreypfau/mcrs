use crate::configuration::{LoadedDimensionTypes, LoadedWorldPreset};
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::chunk::ChunkPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use crate::world::loot::LootPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_block::block_update::BlockUpdatePlugin;
use tracing::{debug, error, info, warn};

pub mod block;
pub mod chunk;
pub mod entity;
mod explosion;
mod format;
mod generate;
mod inventory;
pub mod item;
pub mod loot;
pub mod sub_app_builder;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();
        // Each per-dim sub-app registers `DimensionPlugin` into its own world
        // via `spawn_dim_subapp`; the host world no longer hosts `Dimension`
        // entities, so the plugin is not added here.
        app.add_systems(OnEnter(AppState::Playing), enqueue_dim_spawns_from_preset);
        app.add_plugins(ChunkPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_plugins(ExplosionPlugin);
        app.add_plugins(LootPlugin);
    }
}

/// Enqueue one `DimSpawnRequest` per dimension in the loaded world preset.
///
/// Runs at `OnEnter(AppState::Playing)`, after `WorldgenFreeze` has finished
/// loading registries and the world preset. The outer runner loop drains
/// `DimSpawnQueue` immediately after `app.update()` returns, materialising one
/// per-dim sub-app per request.
pub fn enqueue_dim_spawns_from_preset(
    world_preset: Res<LoadedWorldPreset>,
    dimension_types: Res<LoadedDimensionTypes>,
    mut spawn_queue: ResMut<DimSpawnQueue>,
) {
    if !world_preset.is_loaded {
        // OnEnter(Playing) fires after the WorldgenFreeze → Playing transition;
        // by then the preset must be loaded. Treat the unloaded case as an
        // invariant violation and bail without enqueueing — the server will
        // come up with zero dimensions, which makes the failure mode visible.
        error!(
            "LoadedWorldPreset not loaded when OnEnter(AppState::Playing) fired — \
             expected the WorldgenFreeze chain to ensure preset load completion"
        );
        return;
    }

    if world_preset.dimensions.is_empty() {
        warn!(
            "LoadedWorldPreset has no dimensions, enqueueing default overworld spawn request"
        );
        spawn_queue.0.push(DimSpawnRequest {
            dimension_id: DimensionId::new("minecraft:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
        return;
    }

    debug!(
        preset = %world_preset.preset_name,
        dimension_count = world_preset.dimensions.len(),
        "Enqueueing dimension spawn requests from loaded world preset"
    );

    for (dimension_key, dimension_type_ref) in &world_preset.dimensions {
        let resolved = dimension_types
            .0
            .iter()
            .find(|(id, _)| id.as_str() == dimension_type_ref.as_str())
            .map(|(_, dim_type)| {
                (
                    DimensionTypeConfig::new(dim_type.min_y, dim_type.height),
                    dim_type.has_skylight,
                )
            })
            .unwrap_or_else(|| {
                warn!(
                    dimension_type = %dimension_type_ref,
                    dimension_key = %dimension_key,
                    "Dimension type not found, using default config + has_sky=true"
                );
                (DimensionTypeConfig::default(), true)
            });

        info!(
            dimension_key = %dimension_key,
            dimension_type = %dimension_type_ref,
            min_y = resolved.0.min_y,
            height = resolved.0.height,
            sections = resolved.0.section_count,
            has_skylight = resolved.1,
            "Enqueueing dimension spawn request"
        );

        spawn_queue.0.push(DimSpawnRequest {
            dimension_id: DimensionId::new(dimension_key.as_str()),
            type_config: resolved.0,
            has_sky: resolved.1,
        });
    }

    info!(
        dimension_count = world_preset.dimensions.len(),
        "All dimensions enqueued from world preset"
    );
}
