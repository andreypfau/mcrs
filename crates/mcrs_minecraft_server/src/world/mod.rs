use crate::world_options::DimensionList;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_level::world::sub_app::{DimSpawnQueue, DimSpawnRequest, DimensionPlugins};
use tracing::{debug, info, warn};

pub mod aoi;
pub mod block_entity;
pub mod block_update;
pub mod chunk;
pub mod clock;
pub mod entity;
pub mod generate;
#[cfg(test)]
mod generation_tests;
pub mod light;
pub mod light_codec;
pub mod sub_app_builder;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        // Seeded by `DimensionWorldsPlugin` before this plugin loads; the
        // default is for harnesses that compose `WorldPlugin` on its own.
        app.init_resource::<crate::world_options::WorldSeed>();
        app.add_plugins(mcrs_minecraft_engine::EnginePlugin);
        app.add_plugins(clock::WorldClockHostPlugin);
        app.add_systems(OnEnter(AppState::Playing), enqueue_dim_spawns);
    }
}

/// Enqueue one `DimSpawnRequest` per baked dimension.
///
/// Runs at `OnEnter(AppState::Playing)`. The outer runner loop drains
/// `DimSpawnQueue` immediately after `app.update()` returns, materialising one
/// per-dim sub-app per request.
pub fn enqueue_dim_spawns(
    dimensions: Res<DimensionList>,
    mut spawn_queue: ResMut<DimSpawnQueue>,
    plugins: Option<Res<DimensionPlugins>>,
    mut already_enqueued: Local<bool>,
) {
    if *already_enqueued {
        return;
    }
    let mut unknown: Vec<_> = plugins
        .iter()
        .flat_map(|plugins| plugins.dimensions())
        .filter(|dimension| !dimensions.iter().any(|(key, _)| key == *dimension))
        .collect();
    unknown.sort_unstable_by_key(|dimension| dimension.as_str());
    unknown.dedup();
    for dimension in unknown {
        warn!(
            %dimension,
            "a plugin is added to a dimension that the world does not spawn at start; it gets the plugin only if it spawns later"
        );
    }

    debug!(
        dimension_count = dimensions.len(),
        "Enqueueing dimension spawn requests"
    );

    for (dimension_key, entry) in dimensions.iter() {
        debug!(
            dimension_key = %dimension_key,
            dimension_type = entry.dimension_type.number(),
            "Enqueueing dimension spawn request"
        );

        spawn_queue.0.push(DimSpawnRequest {
            dimension: dimension_key.clone(),
            dimension_type: entry.dimension_type,
        });
    }

    *already_enqueued = true;
    info!(
        dimension_count = dimensions.len(),
        "All dimensions enqueued"
    );
}
