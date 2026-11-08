use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world::sub_app::{
    DimDespawnQueue, DimSpawnEnqueue, DimSpawnQueue, DimSpawnRequest, DimensionPlugins,
};
use tracing::{debug, info, warn};

use crate::banlist::BanListHostPlugin;
use crate::coordinator::birth_clock::BirthClockPlugin;
use crate::coordinator::clock::WorldClockHostPlugin;
use crate::coordinator::dim::DimSubAppHandle;
use crate::coordinator::dim_cells::DimCellsPlugin;
use crate::whitelist::WhitelistHostPlugin;

/// The coordinator: the dimension worlds and their lockstep tick, the queues that spawn and
/// despawn them, the clocks, the whitelist and the ban list.
pub struct CoordinatorPlugin;

impl Plugin for CoordinatorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();
        app.add_plugins(DimCellsPlugin);
        app.add_observer(
            |trigger: On<Remove, DimSubAppHandle>, mut queue: ResMut<DimDespawnQueue>| {
                queue.0.push(trigger.event().entity);
            },
        );
        app.add_plugins((
            WorldClockHostPlugin,
            BirthClockPlugin,
            WhitelistHostPlugin,
            BanListHostPlugin,
        ));
        app.add_systems(
            OnEnter(AppState::Playing),
            enqueue_dim_spawns.in_set(DimSpawnEnqueue),
        );
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

    for (dimension_key, dimension_type) in dimensions.iter() {
        debug!(
            dimension_key = %dimension_key,
            dimension_type = dimension_type.number(),
            "Enqueueing dimension spawn request"
        );

        spawn_queue.0.push(DimSpawnRequest {
            dimension: dimension_key.clone(),
            dimension_type,
        });
    }

    *already_enqueued = true;
    info!(
        dimension_count = dimensions.len(),
        "All dimensions enqueued"
    );
}
