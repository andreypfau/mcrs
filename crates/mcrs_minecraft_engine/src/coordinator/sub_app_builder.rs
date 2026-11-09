use bevy_app::{App, FixedPreUpdate, FixedUpdate, Last, PluginsState, SubApp};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Res;
use mcrs_engine_cell::{CellIndex, Placement};
use tracing::{error, warn};

use crate::assign_global_entity_id::assign_global_entity_id;
use crate::bus::{InboundPlayerSpawn, OutboundPlayerPacket};
use crate::cell_mail::CellMailPlugin;
use crate::command_handoff::CommandHandoff;
use crate::coordinator::dim::DimSubAppHandle;
use crate::coordinator::dim_cells::DimCells;
use crate::coordinator::entity_id_ranges::EntityIdRanges;
use crate::coordinator::running_dimensions::{RunningDimension, RunningDimensions};
use crate::dim_mail::DimMail;
use crate::dim_tick::{DimInbox, DimInboxDrain, drain_dim_inbox, inbox_holds_messages};
use crate::player_order::{DepartureRequests, OrderedPlayer};
use crate::player_roster::{ForeignPlayers, OwnPlayers, report_own_players};
use crate::to_muxer::DimOpened;
use mcrs_minecraft_engine_world::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;

use crate::block_simulation::DimensionBlockRules;
use crate::column_store::ColumnStorePlugin;
use crate::heightmap::{BlockHeightmapPredicates, DimHeightmapPlugin};
use crate::minecraft_entity::MinecraftEntityPlugin;
use crate::player_tracker::PlayerTrackerPlugin;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironment;
use mcrs_minecraft_engine_world::owned_columns::{OwnedColumns, region_partition};
use mcrs_minecraft_engine_world::server_loop::run_schedules_on_one_thread;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::{
    DimensionBundle, DimensionPlugin, DimensionTypeConfig, DimensionTypeId, HasSkyLight, HasWeather,
};
use mcrs_minecraft_engine_world::world::dimension_builders::{
    DimensionBuild, DimensionBuilders, DimensionHostBuilder,
};
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnTraceLog, ColumnTraceSink};
use mcrs_minecraft_engine_world::world::storage::column::ColumnLifecycleSet;
use mcrs_minecraft_engine_world::world::sub_app::{
    DimSpawnQueue, DimSpawnRequest, DimensionPlugins,
};
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_engine_world::world_cells::WorldCells;
use mcrs_minecraft_engine_world::world_placement::WorldPlacement;
use mcrs_minecraft_registries::CurrentRegistries;

/// Builds the worlds of a dimension, one for each of its cells as [`WorldCells`] says, and
/// places them in the host's [`DimCells`]. Returns the label entity of the dimension.
///
/// The label entity comes from the host's allocator, so labels are unique across all worlds:
/// every dimension world would otherwise allocate the same low-index entities. The label
/// entity holds no `Dimension` component. The `Dimension` entity lives inside the world, with the
/// components that the simulation queries.
#[expect(
    clippy::too_many_lines,
    reason = "a dimension's sub-app is assembled in the order its schedules and plugins require, and each step relies on the previous one"
)]
fn spawn_dim_subapp(app: &mut App, request: &DimSpawnRequest) -> Result<Entity, DimSpawnError> {
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
        .write_message(DimOpened { dim: label_entity });
    app.world_mut()
        .get_resource_or_init::<RunningDimensions>()
        .insert(RunningDimension {
            label: label_entity,
            key: request.dimension.clone(),
            dimension_type: request.dimension_type,
        });

    let traces_columns = app.world().contains_resource::<ColumnTraceSink>();
    let cells = app
        .world()
        .get_resource::<WorldCells>()
        .copied()
        .unwrap_or_default();
    let partition = region_partition(cells.cells);
    if cells.cells.get() > 1 {
        app.world_mut()
            .get_resource_or_init::<DimCells>()
            .divide(label_entity, partition);
    }
    // Each cell is a whole dimension world that owns the columns of its regions.
    for index in (0..partition.cells().get()).map(CellIndex) {
        let mut sub_app = SubApp::new();
        mcrs_minecraft_engine_registries::extract_registries(app.world(), sub_app.world_mut());

        sub_app.init_resource::<DimInbox>();
        let owned = (cells.cells.get() > 1).then(|| OwnedColumns::new(partition, index));
        if let Some(owned) = owned {
            sub_app.add_plugins(CellMailPlugin { owned });
        }

        // Only the messages that a dimension writes or reads inside its own world are registered:
        // what crosses to the host goes in the input and the output of a tick.
        sub_app.add_message::<OutboundPlayerPacket>();
        sub_app.add_message::<InboundPlayerSpawn>();
        sub_app.add_message::<crate::bus::InboundPlayerDespawn>();
        sub_app.add_message::<crate::bus::OutboundPlayerReleased>();
        sub_app.add_message::<crate::bus::InboundPlayerPacket>();
        sub_app.add_message::<crate::bus::OutboundPlayerAttached>();
        sub_app.add_message::<crate::departure::PlayerTransfer>();
        sub_app
            .add_message::<mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest>();
        sub_app.add_message::<OrderedPlayer>();
        sub_app.add_message::<CommandHandoff>();
        sub_app.init_resource::<DimMail>();
        sub_app.init_resource::<DepartureRequests>();
        sub_app.init_resource::<ForeignPlayers>();
        sub_app.init_resource::<OwnPlayers>();
        sub_app.add_systems(Last, report_own_players);
        // The block messages live in the dimension world alone, so the host never buffers them.
        sub_app.add_message::<BlockSetRequest>();
        sub_app.add_message::<BlockPlaced>();

        sub_app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        let mut entity_ids = CellEntityIds::default();
        match app
            .world_mut()
            .get_resource_or_init::<EntityIdRanges>()
            .grant()
        {
            Some(range) => entity_ids.add(range),
            None => error!("no global entity ids remain for a new dimension"),
        }
        sub_app.insert_resource(entity_ids);
        sub_app.add_observer(assign_global_entity_id);
        if traces_columns {
            sub_app.init_resource::<ColumnTraceLog>();
        }
        sub_app.insert_resource(mcrs_minecraft_engine_world::world::sub_app::DimensionLabel(
            label_entity,
        ));

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
                .before(crate::column_streams::ColumnStreamSet),
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
            file_path: roots.assets.clone(),
            ..AssetPlugin::default()
        });
        sub_app.add_plugins(PlayerTrackerPlugin);
        let rules = app
            .world()
            .get_resource::<DimensionBlockRules>()
            .copied()
            .unwrap_or_default();
        (rules.0)(&mut sub_app);
        sub_app.add_plugins(MinecraftEntityPlugin);
        sub_app.add_plugins(DimHeightmapPlugin);
        sub_app.add_plugins(ColumnStorePlugin { owned });
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

        // The first tick hands over only what changes after the spawn, so the world starts from
        // the copies of now.
        let host = app.world();
        if let Some(predicates) = host.get_resource::<BlockHeightmapPredicates>() {
            sub_app.insert_resource(predicates.clone());
        }
        if let Some(save) = host.get_resource::<WorldSave>() {
            sub_app.insert_resource(save.clone());
        }
        sub_app.insert_resource(
            host.get_resource::<mcrs_minecraft_environment::WorldClocks>()
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
        sub_app.insert_resource(
            host.get_resource::<crate::whitelist::Whitelist>()
                .cloned()
                .unwrap_or_default(),
        );
        sub_app.insert_resource(
            host.get_resource::<crate::banlist::BanList>()
                .cloned()
                .unwrap_or_default(),
        );
        sub_app.insert_resource(
            host.get_resource::<RunningDimensions>()
                .cloned()
                .unwrap_or_default(),
        );
        sub_app.insert_resource(
            host.get_resource::<mcrs_minecraft_engine_world::game_rules::GameRules>()
                .cloned()
                .unwrap_or_default(),
        );
        if let Some(pace) = host.get_resource::<mcrs_minecraft_engine_world::tick_pace::TickPace>()
        {
            sub_app.insert_resource(pace.clone());
        }
        sub_app.insert_resource(
            host.get_resource::<crate::random_tick_speed::RandomTickSpeed>()
                .copied()
                .unwrap_or_default(),
        );
        if let Some(seed) =
            host.get_resource::<mcrs_minecraft_engine_world::world_seed::WorldSeed>()
        {
            sub_app.insert_resource(*seed);
        }

        // Before the dimension entity spawns, so a plugin can still register required components and
        // hooks for its components and its observers see it spawn.
        let build = DimensionBuild {
            dimension: &request.dimension,
            type_config,
            has_sky_light: has_sky,
        };
        if let Some(builders) = app.world().get_resource::<DimensionBuilders>() {
            builders.build(app.world(), &build, &mut sub_app);
        }
        if index == CellIndex(0) {
            let hosts: Vec<DimensionHostBuilder> = app
                .world()
                .get_resource::<DimensionBuilders>()
                .map(|builders| builders.hosts().to_vec())
                .unwrap_or_default();
            for host in hosts {
                host(app.world_mut(), label_entity, &build, sub_app.world());
            }
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
            index,
            sub_app,
            placement.map_or(Placement::Inline, |placement| placement.0),
        );
    }
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
pub(crate) fn drain_dim_spawn_queue(app: &mut App) {
    let Some(mut queue) = app.world_mut().get_resource_mut::<DimSpawnQueue>() else {
        return;
    };
    let requests: Vec<DimSpawnRequest> = std::mem::take(&mut queue.0);
    if requests.is_empty() {
        return;
    }
    for request in requests {
        if let Err(error) = spawn_dim_subapp(app, &request) {
            error!(%error, "a dimension was not spawned");
        }
    }
}
