use bevy_app::{
    App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
    PluginsState, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, SubApp, Update,
};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Res;
use mcrs_engine_cell::Placement;
use tracing::{error, warn};

use crate::bus::{InboundPlayerSpawn, OutboundPlayerPacket};
use crate::coordinator::dim::DimSubAppHandle;
use crate::coordinator::dim_cells::DimCells;
use crate::dim_tick::{DimInbox, drain_dim_inbox, inbox_holds_messages};
use crate::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};

/// The system set of the inbox drain, early in `FixedPreUpdate`: the messages of the input of a
/// tick reach the dimension here. Systems that consume them run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DimInboxDrain;

use crate::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use crate::heightmap::{BlockHeightmapPredicates, DimHeightmapPlugin};
use crate::minecraft_entity::MinecraftEntityPlugin;
use crate::player_tracker::PlayerTrackerPlugin;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironment;
use mcrs_minecraft_engine_world::server_loop::run_schedules_on_one_thread;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionBundle, DimensionPlugin, DimensionTypeConfig, DimensionTypeId, HasSkyLight, HasWeather,
};
use mcrs_minecraft_engine_world::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnTraceLog, ColumnTraceSink};
use mcrs_minecraft_engine_world::world::storage::column::ColumnLifecycleSet;
use mcrs_minecraft_engine_world::world::sub_app::{
    DimSpawnQueue, DimSpawnRequest, DimensionPlugins,
};
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_engine_world::world_placement::WorldPlacement;
use mcrs_minecraft_heightmap::HeightmapPredicates;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Clone)]
pub struct DimRegistryBundle {
    pub heightmap_predicates: Option<HeightmapPredicates>,
    pub world_save: Option<WorldSave>,
}

pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        heightmap_predicates: world
            .get_resource::<BlockHeightmapPredicates>()
            .map(|predicates| predicates.0.clone()),
        world_save: world.get_resource::<WorldSave>().cloned(),
    }
}

/// Builds the world of a dimension and places it in a cell of the host's [`DimCells`]. Returns
/// the label entity of the dimension.
///
/// The label entity comes from the host's allocator, so labels are unique across all worlds:
/// every dimension world would otherwise allocate the same low-index entities. The label
/// entity holds no `Dimension` component. The `Dimension` entity lives inside the world, with the
/// components that the simulation queries.
#[expect(
    clippy::too_many_lines,
    reason = "a dimension's sub-app is assembled in the order its schedules and plugins require, and each step relies on the previous one"
)]
pub fn spawn_dim_subapp(
    app: &mut App,
    request: &DimSpawnRequest,
    registries: &DimRegistryBundle,
) -> Result<Entity, DimSpawnError> {
    let unknown = || UnknownDimensionType {
        dimension: request.dimension.as_str().to_owned(),
        dimension_type: request.dimension_type.number(),
    };
    let (type_config, has_sky, has_weather) = {
        let file = app
            .world()
            .get_resource::<CurrentRegistries>()
            .and_then(|registries| registries.registry::<DimensionTypeFile>())
            .and_then(|types| {
                let id = mcrs_minecraft_resource::Id::checked(
                    request.dimension_type.number(),
                    types.len(),
                )?;
                types.get(id)
            })
            .ok_or_else(unknown)?;
        let (dimension_type, environment) = file.split();
        (
            DimensionTypeConfig::new(dimension_type.min_y, dimension_type.height),
            dimension_type.has_skylight,
            DimensionEnvironment::of(&request.dimension, &dimension_type, &environment)
                .can_have_weather,
        )
    };

    let roots = app
        .world()
        .get_resource::<mcrs_minecraft_assets::packs::PackRoots>()
        .cloned()
        .ok_or(DimSpawnError::NoPackRoots)?;
    let layering = app
        .world()
        .get_resource::<mcrs_minecraft_assets::packs::PackLayering>()
        .cloned()
        .ok_or(DimSpawnError::NoPackRoots)?;

    let label_entity = app
        .world_mut()
        .spawn((
            DimSubAppHandle,
            request.dimension.clone(),
            DimensionTypeId(request.dimension_type),
        ))
        .id();

    app.world_mut()
        .get_resource_or_init::<DimInputs>()
        .insert(label_entity);

    let traces_columns = app.world().contains_resource::<ColumnTraceSink>();
    let mut sub_app = SubApp::new();
    mcrs_minecraft_engine_registries::extract_registries(app.world(), sub_app.world_mut());

    sub_app.init_resource::<DimInbox>();

    // Only the messages that a dimension writes or reads inside its own world are registered:
    // what crosses to the host goes in the input and the output of a tick.
    sub_app.add_message::<OutboundPlayerPacket>();
    // `InboundPlayerSpawn` is written by `drain_dim_inbox` from `ToDim::Spawn`
    // and read by `consume_inbound_player_spawn` (added by `MinecraftEntityPlugin`).
    sub_app.add_message::<InboundPlayerSpawn>();
    // `InboundPlayerDespawn` is written by `drain_dim_inbox` from `ToDim::Despawn`
    // and read by `drain_inbound_player_despawn` (added by `PlayerTrackerPlugin`).
    sub_app.add_message::<crate::bus::InboundPlayerDespawn>();
    // Written by `despawn_inbound_player` once the player is saved, and handed to the host in
    // the output of the tick.
    sub_app.add_message::<crate::bus::OutboundPlayerReleased>();
    // `InboundPlayerPacket` is read by `dispatch_inbound_to_dim` (added by
    // `MinecraftEntityPlugin`). Serverbound packets arrive via `drain_dim_inbox`.
    sub_app.add_message::<crate::bus::InboundPlayerPacket>();
    // `OutboundPlayerAttached` is written by `consume_inbound_player_spawn` and handed to the
    // host in the output of the tick to attach the session.
    sub_app.add_message::<crate::bus::OutboundPlayerAttached>();
    // Written by `drain_dim_inbox` from `ToDim::Transfer` and read by `receive_transfers`.
    sub_app.add_message::<crate::departure::PlayerTransfer>();
    sub_app.add_message::<mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest>();
    // The sub-app builder is the single source of truth for these registrations — `BlockUpdatePlugin`
    // no longer registers them and instead debug-asserts they are already in
    // place, so a mistaken host-side `add_plugins(BlockUpdatePlugin)` fails
    // loud at plugin load rather than silently exporting the buffers to the
    // host world.
    sub_app.add_message::<BlockSetRequest>();
    sub_app.add_message::<BlockPlaced>();

    sub_app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
    if traces_columns {
        sub_app.init_resource::<ColumnTraceLog>();
    }
    sub_app.insert_resource(mcrs_minecraft_engine_world::world::sub_app::DimensionLabel(
        label_entity,
    ));

    // `init_schedule` keeps a schedule that `SubApp::new` already filled, such as `First` with
    // `message_update_system`.
    sub_app.init_schedule(First);
    sub_app.init_schedule(PreStartup);
    sub_app.init_schedule(Startup);
    sub_app.init_schedule(PostStartup);
    sub_app.init_schedule(PreUpdate);
    sub_app.init_schedule(FixedFirst);
    sub_app.init_schedule(FixedPreUpdate);
    sub_app.init_schedule(FixedUpdate);
    sub_app.init_schedule(FixedPostUpdate);
    sub_app.init_schedule(FixedLast);
    sub_app.init_schedule(Update);
    sub_app.init_schedule(PostUpdate);
    sub_app.init_schedule(Last);

    sub_app.add_systems(
        FixedPreUpdate,
        drain_dim_inbox
            .run_if(inbox_holds_messages)
            .in_set(DimInboxDrain),
    );
    sub_app.add_systems(
        FixedUpdate,
        // A column packet carries the heightmaps, so they land before the columns go out.
        crate::heightmap::prime_column_heightmaps
            .run_if(heightmaps_pending)
            .in_set(TickSet::Project)
            .after(ColumnLifecycleSet)
            .before(crate::column_view::ColumnViewSet),
    );
    sub_app.add_plugins(DimensionPlugin);
    // AssetPlugin and AppTypeRegistry must precede any plugin that calls
    // `init_asset` / `register_asset_loader`. `AppTypeRegistry` is initialised by `App::new` but
    // not by `SubApp::new`, so the sub-app needs the explicit `init_resource` call.
    sub_app.init_resource::<bevy_ecs::reflect::AppTypeRegistry>();
    // Dropping a notify fsevents watcher joins its CFRunLoop thread and can
    // block forever; one recursive watch over the 22k-file corpus per dimension
    // also costs more than the whole sub-app spawn. Nothing here hot-reloads.
    sub_app
        .world_mut()
        .get_resource_or_init::<bevy_asset::io::AssetSourceBuilders>()
        .insert(
            bevy_asset::io::AssetSourceId::Default,
            mcrs_minecraft_assets::packs::layered_file_source(&roots, &layering),
        );
    sub_app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        file_path: roots.assets,
        ..AssetPlugin::default()
    });
    sub_app.add_plugins(PlayerTrackerPlugin);
    sub_app.add_plugins(BlockUpdatePlugin);
    sub_app.add_plugins(BlockUpdateWirePlugin);
    sub_app.add_plugins(MinecraftEntityPlugin);
    sub_app.add_plugins(DimHeightmapPlugin);
    let default_op_level = app
        .world()
        .get_resource::<crate::coordinator::ops::DefaultOpLevel>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(default_op_level);
    let default_game_mode = app
        .world()
        .get_resource::<crate::coordinator::default_game_mode::DefaultGameMode>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(default_game_mode);
    if let Some(predicates) = &registries.heightmap_predicates {
        sub_app.insert_resource(BlockHeightmapPredicates(predicates.clone()));
    }
    if let Some(save) = &registries.world_save {
        sub_app.insert_resource(save.clone());
    }

    // The first tick hands over only what changes after the spawn, so the world starts from
    // the copies of now.
    let host = app.world();
    sub_app.insert_resource(
        host.get_resource::<mcrs_minecraft_environment::world_clock::WorldClocks>()
            .cloned()
            .unwrap_or_default(),
    );
    sub_app.insert_resource(
        host.get_resource::<crate::coordinator::clock::GameTime>()
            .copied()
            .unwrap_or_default(),
    );
    sub_app.insert_resource(
        host.get_resource::<crate::coordinator::ops::OpList>()
            .cloned()
            .unwrap_or_default(),
    );

    // Before the dimension entity spawns, so a plugin can still register required components and
    // hooks for its components and its observers see it spawn.
    if let Some(builders) = app.world().get_resource::<DimensionBuilders>() {
        let build = DimensionBuild {
            dimension: &request.dimension,
            type_config,
            has_sky_light: has_sky,
        };
        builders.build(app.world(), &build, &mut sub_app);
    }
    if let Some(plugins) = app.world().get_resource::<DimensionPlugins>() {
        plugins.build(&request.dimension, &mut sub_app);
    }

    let dim_entity = sub_app
        .world_mut()
        .spawn((
            DimensionBundle::new(request.dimension.clone(), type_config),
            DimensionTypeId(request.dimension_type),
        ))
        .id();
    if has_sky {
        sub_app
            .world_mut()
            .entity_mut(dim_entity)
            .insert(HasSkyLight);
    }
    if has_weather {
        sub_app
            .world_mut()
            .entity_mut(dim_entity)
            .insert(HasWeather);
    }

    // Drain plugins to Ready before finish/cleanup. The built-in plugins are
    // ready at once. A plugin from `DimensionPlugins` that overrides
    // `Plugin::ready` blocks the server loop here, and no schedule of this
    // world runs while it waits: its readiness must come from the task pools.
    let waiting = std::time::Instant::now();
    let mut warned = false;
    while sub_app.plugins_state() == PluginsState::Adding {
        bevy_tasks::tick_global_task_pools_on_main_thread();
        std::thread::yield_now();
        if !warned && waiting.elapsed() > std::time::Duration::from_secs(10) {
            warn!(
                dim = request.dimension.as_str(),
                "a plugin of this dimension is not ready after 10 s; the server waits for it"
            );
            warned = true;
        }
    }
    sub_app.finish();
    sub_app.cleanup();
    let placement = app.world().get_resource::<WorldPlacement>().copied();
    if placement == Some(WorldPlacement(Placement::Inline)) {
        run_schedules_on_one_thread(sub_app.world_mut());
    }
    app.world_mut().get_resource_or_init::<DimCells>().add(
        label_entity,
        sub_app,
        placement.map_or(Placement::Inline, |placement| placement.0),
    );
    Ok(label_entity)
}

fn heightmaps_pending(pending: Res<crate::heightmap::PendingColumnHeightmaps>) -> bool {
    !pending.0.is_empty()
}

#[derive(Debug, thiserror::Error)]
#[error(
    "dimension {dimension} names dimension type {dimension_type}, which the loaded registry does not hold"
)]
pub struct UnknownDimensionType {
    pub dimension: String,
    pub dimension_type: u16,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DimSpawnError {
    #[error(transparent)]
    UnknownDimensionType(#[from] UnknownDimensionType),
    #[error("the host registered no pack roots for a dimension's assets to read")]
    NoPackRoots,
}

/// Builds a dimension world for each request of the host's `DimSpawnQueue`. The host calls it
/// between ticks.
pub fn drain_dim_spawn_queue(app: &mut App) {
    let Some(mut queue) = app.world_mut().get_resource_mut::<DimSpawnQueue>() else {
        return;
    };
    let requests: Vec<DimSpawnRequest> = std::mem::take(&mut queue.0);
    if requests.is_empty() {
        return;
    }
    let bundle = gather_dim_registries(app.world());
    for request in requests {
        if let Err(error) = spawn_dim_subapp(app, &request, &bundle) {
            error!(%error, "a dimension was not spawned");
        }
    }
}
