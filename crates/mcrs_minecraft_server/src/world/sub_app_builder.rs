use crate::loaded::Loaded;
use bevy_app::{
    App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
    PluginsState, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, SubApp, Update,
};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel, SystemSet};
use bevy_ecs::system::{Local, Res, ResMut};
use bevy_ecs::world::World;
use bevy_time::{Fixed, Real, Time, Virtual};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;
use std::collections::VecDeque;
use tracing::{debug, error, warn};

use crate::world::bus::{
    InboundConfirmMove, InboundEntitySpawn, InboundPlayerDespawn, InboundPlayerSpawn,
    InboundRollbackMove, OutboundPlayerPacket,
};
use crate::world::channel_types::{FromDim, ToDim};
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft_level::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_level::world::channels::{
    DimChannels, FROM_DIM_CAPACITY, FromDimSender, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    ToDimReceiver,
};

/// System set for inbox drain systems, run early in `FixedPreUpdate`.
/// Arrival and other systems that consume inbound messages run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DimInboxDrain;

/// Private driver schedule: Bevy's `SubApp::run_default_schedule` invokes only
/// the single schedule pointed at by `update_schedule`. This schedule chains
/// the same Bevy stock schedules that `Main` chains in a regular `App`, so
/// every per-dim plugin runs the same systems it would in a single-app
/// composition:
///
/// - Startup family (`PreStartup`, `Startup`, `PostStartup`) runs once on the
///   first pump, guarded by an internal `Local<bool>`.
/// - Each subsequent pump runs `First → PreUpdate → FixedFirst → FixedPreUpdate
///   → FixedUpdate → FixedPostUpdate → FixedLast → Update → PostUpdate → Last`
///   exactly once.
///
/// Two intentional differences from Bevy's stock `Main`:
/// - No `RunFixedMainLoop` indirection. The host runner pumps each sub-app
///   exactly once per host tick, so the Fixed* schedules run unconditionally
///   each pump rather than being driven by accumulated `Time<Fixed>`. The host
///   itself owns the fixed-timestep cadence; the sub-app mirrors it 1:1.
/// - No `SpawnScene` (we do not depend on `bevy_scene`).
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

/// Takes the columns whose sections landed on to the wire: run at the end of every tick and
/// again between ticks, so a column read from the save is not held for the next tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnDrain;

/// The stages of a [`ColumnDrain`], in the order they run. Every one only acts on what is
/// pending, which is what lets the drain run between ticks as well as at their end.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnDrainSet {
    /// Finished generation and saved columns come off the pool into their sections.
    Collect,
    /// The column index, block entities and heightmaps catch up with what landed.
    Reconcile,
    /// Ticket levels spread, sections spawn, and the columns they need are queued and dispatched.
    Request,
    /// A finished light epoch is retired and the next one dispatched.
    Light,
    /// Ready columns go out and the outbox is flushed towards the host.
    Send,
}
use crate::WorldSave;
use crate::world::aoi::PlayerTrackerPlugin;
use crate::world::block::tnt::TntBlockPlugin;
use crate::world::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::generate::DimensionRouters;
use crate::world::heightmap::DimHeightmapPlugin;
use crate::world::light::DimLightPlugin;
use crate::world::loot::LootPlugin;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeEnvironment;
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironment;
use mcrs_minecraft_level::explosion::ExplosionPlugin;
use mcrs_minecraft_level::world::dimension::{
    DimensionBundle, DimensionPlugin, DimensionTypeConfig, DimensionTypeId, HasSkyLight, HasWeather,
};
use mcrs_minecraft_level::world::lifecycle::trace::{ColumnTraceLog, ColumnTraceSink};
use mcrs_minecraft_level::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_registry::shared::{Resolved, SharedRegistries};
use mcrs_minecraft_worldgen_generator::SurfaceIds;
use mcrs_minecraft_worldgen_generator::heightmap::HeightmapPredicates;
use mcrs_minecraft_worldgen_generator::ids::FillIds;
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::PresetBiomeTables;
use mcrs_minecraft_worldgen_generator::saved::SavedColumns;
use mcrs_minecraft_worldgen_generator::stages::{FillContext, dimension_y_sections};

#[derive(Clone)]
pub struct DimRegistryBundle {
    pub light_registry: Option<std::sync::Arc<mcrs_minecraft_light::block::LightRegistry>>,
    pub heightmap_predicates: Option<HeightmapPredicates>,
    pub modern_carver_biomes: crate::world::generate::modern_carvers::DimensionCarverBiomes,
    pub features: crate::world::generate::features::DimensionFeaturePrograms,
    pub structures: crate::world::generate::structures::DimensionStructures,
    pub world_save: Option<WorldSave>,
    pub noise_routers: DimensionRouters,
}

pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        light_registry: world
            .get_resource::<mcrs_minecraft_light::block_light::BlockLightRegistry>()
            .map(|registry| registry.0.clone()),
        heightmap_predicates: world.get_resource::<HeightmapPredicates>().cloned(),
        modern_carver_biomes: world
            .get_resource::<crate::world::generate::modern_carvers::DimensionCarverBiomes>()
            .cloned()
            .unwrap_or_default(),
        features: world
            .get_resource::<crate::world::generate::features::DimensionFeaturePrograms>()
            .cloned()
            .unwrap_or_default(),
        structures: world
            .get_resource::<crate::world::generate::structures::DimensionStructures>()
            .cloned()
            .unwrap_or_default(),
        world_save: world.get_resource::<WorldSave>().cloned(),
        noise_routers: world
            .get_resource::<DimensionRouters>()
            .cloned()
            .unwrap_or_default(),
    }
}

/// Materialise a per-dimension sub-app and return the `DimAppLabel` key as
/// an `Entity`.
///
/// Constraints encoded here:
/// - `update_schedule` is set to `DimTick` so Bevy's
///   `SubApp::run_default_schedule` invokes `DimTick`, which chains
///   `FixedFirst → FixedPreUpdate → FixedUpdate → FixedPostUpdate → FixedLast`
///   exactly once per host pump. This ensures all per-dim plugin systems
///   execute rather than only `FixedUpdate`.
/// - The label `Entity` is allocated from the host world's `Entities`
///   allocator so labels are globally unique across all sub-apps. Each
///   sub-app `World` would otherwise allocate the same low-index `Entity`
///   value, which would collide in the `DimAppLabel(Entity)` interned key.
///   The host world does not hold a `Dimension`-tagged entity — the label
///   entity is reserved (no `Dimension` component) and exists only to
///   anchor the label.
/// - A separate `Dimension` entity lives inside the sub-app's `World`,
///   carrying the per-dim components that the simulation queries against.
pub fn spawn_dim_subapp(
    app: &mut App,
    request: &DimSpawnRequest,
    registries: &DimRegistryBundle,
) -> Result<Entity, UnknownDimensionType> {
    let unknown = || UnknownDimensionType {
        dimension: request.dimension.as_str().to_owned(),
        dimension_type: request.dimension_type.number(),
    };
    let (type_config, has_sky, has_weather) = {
        let set = app.world().get_resource::<RegistrySet>();
        let types = set.and_then(|set| set.entries::<DimensionType, DimensionType>());
        let environments =
            set.and_then(|set| set.entries::<DimensionType, DimensionTypeEnvironment>());
        let dimension_type = types
            .as_ref()
            .and_then(|types| types.get(request.dimension_type))
            .ok_or_else(unknown)?;
        let environment = environments
            .as_ref()
            .and_then(|environments| environments.get(request.dimension_type))
            .ok_or_else(unknown)?;
        (
            DimensionTypeConfig::new(dimension_type.min_y, dimension_type.height),
            dimension_type.has_skylight,
            DimensionEnvironment::of(&request.dimension, dimension_type, environment)
                .can_have_weather,
        )
    };

    let label_entity = app
        .world_mut()
        .spawn((DimSubAppHandle, request.dimension.clone()))
        .id();

    let (to_dim_srv_tx, to_dim_srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (to_dim_ctl_tx, to_dim_ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_dim_tx, from_dim_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);

    app.world_mut()
        .resource_mut::<DimChannels<ToDim, FromDim>>()
        .insert(label_entity, to_dim_srv_tx, to_dim_ctl_tx, from_dim_rx);

    let asset_root = app
        .world()
        .get_resource::<mcrs_minecraft_level::server_loop::AssetRoot>()
        .cloned()
        .unwrap_or_default()
        .0;

    let column_traces = app.world().get_resource::<ColumnTraceSink>().cloned();
    let trace_dimension = request.dimension.as_str().to_owned();
    let biome_source = app
        .world()
        .get_resource::<crate::world_options::DimensionList>()
        .and_then(|list| list.iter().find(|(key, _)| **key == request.dimension))
        .and_then(|(_, entry)| match &entry.generator {
            ChunkGenerator::Noise(generator) => {
                Some(std::sync::Arc::new(generator.biome_source.clone()))
            }
            _ => None,
        });

    let mut sub_app = SubApp::new();
    if let Some(shared) = app.world().get_resource::<SharedRegistries>() {
        shared
            .copy_into(app.world(), sub_app.world_mut())
            .unwrap_or_else(|missing| panic!("{missing}"));
    }

    sub_app.insert_resource(ToDimReceiver::<ToDim> {
        serverbound: to_dim_srv_rx,
        control: to_dim_ctl_rx,
    });
    sub_app.insert_resource(FromDimSender::<FromDim>(from_dim_tx));

    // Per-sub-app message registrations. Only types that still flow through
    // the dim sub-app's Messages<T> double-buffer (intra-dim use) are kept.
    // Types that previously crossed the host↔dim boundary via the extract
    // closure are now carried by the ToDim/FromDim flume channels instead
    // and must NOT be registered here (a stray registration leaves an
    // undrained double-buffer that retains messages across ticks).
    sub_app.add_message::<OutboundPlayerPacket>();
    // `InboundPlayerSpawn` is written by `drain_to_dim_inbox` from `ToDim::Spawn`
    // and read by `consume_inbound_player_spawn` (added by `MinecraftEntityPlugin`).
    sub_app.add_message::<InboundPlayerSpawn>();
    // `InboundPlayerDespawn` is written by `drain_to_dim_inbox` from `ToDim::Despawn`
    // and read by `drain_inbound_player_despawn` (added by `PlayerTrackerPlugin`).
    sub_app.add_message::<crate::world::bus::InboundPlayerDespawn>();
    // Written by `despawn_inbound_player` once the player is saved and drained into the
    // `FromDim` channel by `flush_from_dim_outbox`.
    sub_app.add_message::<crate::world::bus::OutboundPlayerReleased>();
    // `InboundPlayerPacket` is read by `dispatch_inbound_to_dim` (added by
    // `MinecraftEntityPlugin`). Serverbound packets arrive via `drain_to_dim_inbox`.
    sub_app.add_message::<crate::world::bus::InboundPlayerPacket>();
    // `OutboundPlayerAttached` is written by `consume_inbound_player_spawn` and
    // extracted by the host to attach the session.
    sub_app.add_message::<crate::world::bus::OutboundPlayerAttached>();
    // Confirmed-move inbound messages drained from the control channel and read
    // by the arrival systems (SpawnEntity) and source-dim systems (ConfirmMove,
    // RollbackMove).
    sub_app.add_message::<InboundEntitySpawn>();
    sub_app.add_message::<InboundConfirmMove>();
    sub_app.add_message::<InboundRollbackMove>();
    // `PlayerWillDestroyBlock` still flows intra-dim: the per-dim TNT plugin
    // reads it via MessageReader.
    sub_app.add_message::<PlayerWillDestroyBlock>();
    // `ExplosionPlugin::tick_explode` writes `MessageWriter<BlockSetRequest>`
    // and `apply_voxel_set_requests` reads the same buffer;
    // both plugins now live in this per-dim sub-app so the explosion ->
    // block-set chain runs as a single message hop. The sub-app builder is
    // the single source of truth for these registrations — `BlockUpdatePlugin`
    // no longer registers them and instead debug-asserts they are already in
    // place, so a mistaken host-side `add_plugins(BlockUpdatePlugin)` fails
    // loud at plugin load rather than silently exporting the buffers to the
    // host world.
    sub_app.add_message::<BlockSetRequest>();
    sub_app.add_message::<BlockPlaced>();

    sub_app.init_resource::<mcrs_minecraft_level::session::DimPlayerIndex>();
    if column_traces.is_some() {
        sub_app.init_resource::<ColumnTraceLog>();
    }
    sub_app.insert_resource(mcrs_minecraft_level::world::in_flight::MoveIds::new(
        label_entity,
    ));

    sub_app.update_schedule = Some(DimTick.intern());
    sub_app.add_schedule(Schedule::new(DimTick));
    sub_app.add_schedule(Schedule::new(First));
    sub_app.add_schedule(Schedule::new(PreStartup));
    sub_app.add_schedule(Schedule::new(Startup));
    sub_app.add_schedule(Schedule::new(PostStartup));
    sub_app.add_schedule(Schedule::new(PreUpdate));
    sub_app.add_schedule(Schedule::new(FixedFirst));
    sub_app.add_schedule(Schedule::new(FixedPreUpdate));
    sub_app.add_schedule(Schedule::new(FixedUpdate));
    sub_app.add_schedule(Schedule::new(FixedPostUpdate));
    sub_app.add_schedule(Schedule::new(FixedLast));
    sub_app.add_schedule(Schedule::new(Update));
    sub_app.add_schedule(Schedule::new(PostUpdate));
    sub_app.add_schedule(Schedule::new(Last));
    #[cfg(feature = "telemetry-tracy")]
    let dim_for_tick = request.dimension.to_string();
    #[cfg(feature = "telemetry-tracy")]
    let dim_for_extract = request.dimension.to_string();
    sub_app.add_systems(
        DimTick,
        move |world: &mut World, mut startup_done: Local<bool>| {
            #[cfg(feature = "telemetry-tracy")]
            let _dim_span = tracing::info_span!("dim_tick", dim = %dim_for_tick).entered();
            if !*startup_done {
                world.run_schedule(PreStartup);
                world.run_schedule(Startup);
                world.run_schedule(PostStartup);
                *startup_done = true;
            }
            world.run_schedule(First);
            world.run_schedule(PreUpdate);
            world.run_schedule(FixedFirst);
            world.run_schedule(FixedPreUpdate);
            world.run_schedule(FixedUpdate);
            world.run_schedule(FixedPostUpdate);
            world.run_schedule(FixedLast);
            world.run_schedule(Update);
            world.run_schedule(PostUpdate);
            world.run_schedule(Last);
        },
    );

    sub_app.add_systems(FixedPreUpdate, drain_to_dim_inbox.in_set(DimInboxDrain));
    sub_app.add_schedule(Schedule::new(ColumnDrain));
    sub_app.configure_sets(
        ColumnDrain,
        (
            ColumnDrainSet::Collect,
            ColumnDrainSet::Reconcile,
            ColumnDrainSet::Request,
            ColumnDrainSet::Light,
            ColumnDrainSet::Send,
        )
            .chain(),
    );
    sub_app.add_systems(
        ColumnDrain,
        (
            (
                crate::world::chunk::process_completed_columns,
                crate::world::chunk::deliver_merged_columns,
            )
                .chain()
                .in_set(ColumnDrainSet::Collect),
            // The light packet walks the column index, which is rebuilt here rather than left
            // to the tick: a column sent before its sections are in it goes out unlit.
            (
                mcrs_minecraft_level::world::storage::column::reconcile_columns,
                mcrs_minecraft_level::world::storage::block_entity::reconcile_block_entities,
                crate::world::heightmap::prime_column_heightmaps,
            )
                .chain()
                .in_set(ColumnDrainSet::Reconcile),
            (
                mcrs_minecraft_level::world::lifecycle::ticket::propagate_section_levels,
                mcrs_minecraft_level::world::lifecycle::ticket::spawn_chunks,
                crate::world::chunk::enqueue_pending_columns,
                crate::world::chunk::dispatch_column_generation
                    .run_if(bevy_ecs::prelude::resource_exists::<FillContext>),
            )
                .chain()
                .in_set(ColumnDrainSet::Request),
            // A finished epoch frees its slot and unblocks its area only when it is
            // retired, so retiring once a tick leaves most of the light engine's
            // concurrency idle while columns arrive many times a tick. Retire and
            // re-dispatch here, where the columns land, and a column's light is
            // published in time for the send that follows it in this same drain.
            (
                mcrs_minecraft_light::prelude::publish_light,
                mcrs_minecraft_light::prelude::dispatch_epoch,
            )
                .chain()
                .run_if(
                    bevy_ecs::prelude::resource_exists::<mcrs_minecraft_light::prelude::Lighting>,
                )
                .in_set(ColumnDrainSet::Light),
            (
                crate::world::entity::player::column_view::project_touched_columns,
                crate::world::entity::player::column_view::send_column_queue,
                crate::world::aoi::mirror_held_columns,
                flush_from_dim_outbox,
            )
                .chain()
                .in_set(ColumnDrainSet::Send),
        ),
    );
    sub_app.add_systems(FixedLast, |world: &mut World| {
        world.run_schedule(ColumnDrain)
    });

    sub_app.add_plugins(DimensionPlugin);
    // AssetPlugin and AppTypeRegistry must precede any plugin that calls
    // `init_asset` / `register_asset_loader`. `ChunkPlugin` (via its nested
    // `NoiseGeneratorSettingsPlugin`) registers assets, so it must come after
    // this block. `AppTypeRegistry` is initialised by `App::new` but not by
    // `SubApp::new`, so the sub-app needs the explicit `init_resource` call.
    sub_app.init_resource::<bevy_ecs::reflect::AppTypeRegistry>();
    // Dropping a notify fsevents watcher joins its CFRunLoop thread and can
    // block forever; one recursive watch over the 22k-file corpus per dimension
    // also costs more than the whole sub-app spawn. Nothing here hot-reloads.
    sub_app
        .world_mut()
        .get_resource_or_init::<bevy_asset::io::AssetSourceBuilders>()
        .insert(
            bevy_asset::io::AssetSourceId::Default,
            mcrs_minecraft_assets::packs::layered_file_source(
                &asset_root,
                mcrs_minecraft_worldgen_builtin::asset,
            ),
        );
    sub_app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        file_path: asset_root,
        ..AssetPlugin::default()
    });
    // The worldgen `ChunkPlugin` (ColumnScheduler, the CHUNK_TASK_POOL, and the
    // five FixedPreUpdate worldgen systems) is the per-dim entry-point that
    // turns DimSpawnRequest into populated columns. It is distinct from the
    // engine-level `storage::section::SectionPlugin` that DimensionPlugin adds
    // (which only contributes TicketPlugin).
    //
    // The router is compiled host-side and arrives here as a read-only
    // snapshot; a dimension the preset drives with no noise generator simply
    // gets none, and `dispatch_column_generation` never runs for it.
    let dimension = request.dimension.location();
    match registries.noise_routers.0.get(dimension) {
        Some(dimension_router) => {
            let router = &dimension_router.router;
            let blocks = sub_app.world().resource::<Blocks>().0.clone();
            let block_tags = sub_app
                .world()
                .resource::<RegistrySet>()
                .loaded_tags::<Block>();
            let surface_ids = sub_app.world().resource::<Resolved<SurfaceIds>>().clone();
            let fill_ids = sub_app.world().resource::<Resolved<FillIds>>().clone();
            let preset_tables = sub_app
                .world()
                .resource::<Resolved<PresetBiomeTables>>()
                .clone();
            let features = registries
                .features
                .0
                .get(dimension)
                .map(std::sync::Arc::clone);
            // A dimension with no program decorates nothing, which is silent
            // in the world and loud only here.
            if features.is_none() {
                warn!(%dimension, "no feature program for this dimension; it will place no features");
            }
            sub_app.insert_resource(FillContext::build(
                std::sync::Arc::clone(router),
                Some(std::sync::Arc::clone(&dimension_router.material)),
                blocks,
                dimension_y_sections(router, type_config.min_y, type_config.section_count),
                biome_source,
                &preset_tables,
                registries.heightmap_predicates.clone(),
                registries.world_save.as_ref().and_then(|save| {
                    let set = sub_app.world().resource::<RegistrySet>().clone();
                    SavedColumns::open(&save.0, &request.dimension, set)
                }),
                registries
                    .modern_carver_biomes
                    .0
                    .get(dimension)
                    .map(std::sync::Arc::clone),
                &block_tags,
                &surface_ids,
                &fill_ids,
                features,
                registries.structures.0.get(dimension).cloned(),
            ));
        }
        None => {
            warn!(%dimension, "no noise router for this dimension; it will generate nothing")
        }
    }
    sub_app.add_plugins(crate::world::chunk::ChunkPlugin);
    // Per-dim composition of the simulation plugins. Each plugin's
    // schedule placements (`TntBlockPlugin`, `ExplosionPlugin`,
    // `PlayerTrackerPlugin`, `BlockUpdatePlugin`, `BlockUpdateWirePlugin`,
    // `MinecraftEntityPlugin`, `LootPlugin`) run inside the per-dim
    // sub-app's `World`. `ExplosionPlugin::tick_explode` writes
    // `MessageWriter<BlockSetRequest>`; `BlockUpdatePlugin`'s
    // `apply_voxel_set_requests` reads the same per-dim buffer in the
    // same world, restoring the single-hop block-update path. The
    // additional `BlockUpdateWirePlugin` (defined in
    // `crate::world::block_update`) registers the per-dim wire-emit
    // system that fans block updates out via `OutboundPlayerPacket`
    // through `Column.PlayerObservers`. `TntBlockPlugin` reads
    // `MessageReader<PlayerWillDestroyBlock>` — the host-side
    // `digging.rs` writers route a clone of each event into
    // `PendingInboundLifecycle.block_events`, drained per-dim by the
    // extract closure below.
    sub_app.add_plugins(TntBlockPlugin);
    sub_app.add_plugins(ExplosionPlugin);
    sub_app.add_plugins(PlayerTrackerPlugin);
    sub_app.add_plugins(BlockUpdatePlugin::default());
    sub_app.add_plugins(BlockUpdateWirePlugin);
    sub_app.add_plugins(crate::world::item::ItemPlugin);
    sub_app.add_plugins(MinecraftEntityPlugin);
    sub_app.add_plugins(LootPlugin);
    sub_app.add_plugins(mcrs_minecraft_level::experience::ExperiencePlugin);
    sub_app.add_plugins(crate::world::arrival::ArrivalPlugin);
    sub_app.add_plugins(DimHeightmapPlugin);
    let lighting = app
        .world()
        .get_resource::<crate::Lighting>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(lighting);
    let default_op_level = app
        .world()
        .get_resource::<crate::ops::DefaultOpLevel>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(default_op_level);
    let default_game_mode = app
        .world()
        .get_resource::<crate::world::entity::player::DefaultGameMode>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(default_game_mode);
    let slow_column_threshold = app
        .world()
        .get_resource::<crate::world::chunk::SlowColumnThreshold>()
        .copied()
        .unwrap_or_default();
    sub_app.insert_resource(slow_column_threshold);
    if let Some(registry) = &registries.light_registry {
        sub_app.add_plugins(DimLightPlugin {
            registry: std::sync::Arc::clone(registry),
            bounds: mcrs_minecraft_light::level::LightBounds::from_dimension(
                type_config.min_y,
                type_config.section_count,
            ),
            sky: has_sky,
        });
    } else if lighting == crate::Lighting::Propagated {
        warn!(
            dim = request.dimension.as_str(),
            "no block light table; this dimension will publish no light"
        );
    }

    if let Some(predicates) = &registries.heightmap_predicates {
        sub_app.insert_resource(predicates.clone());
    }
    sub_app.insert_resource(registries.structures.clone());
    if let Some(save) = &registries.world_save {
        sub_app.insert_resource(save.clone());
    }

    // Seed the time resources so an inspector that reads `Res<Time<…>>` on a
    // sub-app that has never been pumped gets a valid default. The extract
    // closure overwrites these every subsequent tick.
    sub_app.insert_resource(Time::<Fixed>::default());
    sub_app.insert_resource(Time::<Virtual>::default());
    sub_app.insert_resource(Time::<Real>::default());
    sub_app.init_resource::<mcrs_minecraft_environment::world_clock::WorldClocks>();
    sub_app.init_resource::<crate::ops::OpList>();

    sub_app.set_extract(move |main_world, sub_world| {
        use crate::world::bus::OutboundPlayerAttached;

        #[cfg(feature = "telemetry-tracy")]
        let _dim_span = tracing::info_span!("dim_extract", dim = %dim_for_extract).entered();
        if let Some(time_fixed) = main_world.get_resource::<Time<Fixed>>() {
            sub_world.insert_resource(*time_fixed);
        }
        if let Some(time_virtual) = main_world.get_resource::<Time<Virtual>>() {
            sub_world.insert_resource(*time_virtual);
        }
        if let Some(time_real) = main_world.get_resource::<Time<Real>>() {
            sub_world.insert_resource(*time_real);
        }
        if let Some(time) = main_world.get_resource::<Time<()>>() {
            sub_world.insert_resource(*time);
        }
        mcrs_minecraft_environment::world_clock::extract_world_clocks(main_world, sub_world);
        if let Some(ops) = main_world.get_resource::<crate::ops::OpList>() {
            sub_world.insert_resource(ops.clone());
        }
        if let Some(traces) = &column_traces
            && let Some(mut log) = sub_world.get_resource_mut::<ColumnTraceLog>()
        {
            traces.record(&trace_dimension, log.drain());
        }

        // Also extract OutboundPlayerAttached written directly to the sub-app Messages
        // (i.e., before flush_from_dim_outbox drains it). This covers the case where
        // consume_inbound_player_spawn writes OutboundPlayerAttached but it hasn't been
        // flushed yet (e.g., if the sub-app update hasn't reached FixedLast).
        let attached: Vec<OutboundPlayerAttached> = sub_world
            .get_resource_mut::<Messages<OutboundPlayerAttached>>()
            .map(|mut m| m.drain().collect())
            .unwrap_or_default();
        if !attached.is_empty()
            && let Some(mut host_msgs) =
                main_world.get_resource_mut::<Messages<OutboundPlayerAttached>>()
        {
            for msg in attached {
                host_msgs.write(msg);
            }
        }
    });

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

    // Drain plugins to Ready before finish/cleanup. All plugins currently
    // composed into a per-dim sub-app reach PluginsState::Ready synchronously
    // (none override Plugin::ready), so this loop exits immediately. It is
    // retained to make the contract explicit: any future plugin that introduces
    // async readiness (e.g., per-dim biome asset loading) will be correctly
    // waited on here rather than silently breaking sub-app construction.
    while sub_app.plugins_state() == PluginsState::Adding {
        bevy_tasks::tick_global_task_pools_on_main_thread();
    }
    sub_app.finish();
    sub_app.cleanup();

    app.insert_sub_app(DimAppLabel(label_entity), sub_app);
    Ok(label_entity)
}

/// Drains both `ToDim` channel receivers into the dim's local message buses.
/// The control channel (Spawn/Despawn/Attach/SpawnEntity/ConfirmMove/RollbackMove)
/// is drained first so lifecycle messages always precede any same-tick Serverbound
/// packet for a freshly transferred player.
fn drain_to_dim_inbox(
    rx: Res<ToDimReceiver<ToDim>>,
    mut spawn_msgs: ResMut<Messages<InboundPlayerSpawn>>,
    mut despawn_msgs: ResMut<Messages<InboundPlayerDespawn>>,
    mut serverbound_msgs: ResMut<Messages<crate::world::bus::InboundPlayerPacket>>,
    mut entity_spawn_msgs: ResMut<Messages<InboundEntitySpawn>>,
    mut confirm_msgs: ResMut<Messages<InboundConfirmMove>>,
    mut rollback_msgs: ResMut<Messages<InboundRollbackMove>>,
) {
    for msg in rx.control.try_iter() {
        match msg {
            ToDim::Spawn(m) => {
                spawn_msgs.write(m);
            }
            ToDim::Despawn(m) => {
                despawn_msgs.write(m);
            }
            ToDim::Serverbound(_) => {}
            ToDim::SpawnEntity(m) => {
                entity_spawn_msgs.write(m);
            }
            ToDim::ConfirmMove(m) => {
                confirm_msgs.write(m);
            }
            ToDim::RollbackMove(m) => {
                rollback_msgs.write(m);
            }
        }
    }
    for msg in rx.serverbound.try_iter() {
        if let ToDim::Serverbound(m) = msg {
            serverbound_msgs.write(m);
        }
    }
}

/// Drains the dim's local `Messages<OutboundPlayerPacket>` into the `FromDim`
/// channel so the host-side `pump_channels` can epoch-stamp and forward them.
///
/// A tick can emit far more packets than the channel holds — a chunk batch at a
/// high `desired_columns_per_tick` does it routinely — so a full channel is
/// backpressure, not a fault, and what does not fit waits here for the next
/// tick in the order it was written. Nothing is ever dropped: a column is
/// recorded as sent the moment it is queued and never offered again, so a lost
/// packet is a hole in the client's world, and a lost batch-finished packet
/// costs the acknowledgement the whole column stream is paced by.
pub(crate) fn flush_from_dim_outbox(
    mut msgs: ResMut<Messages<OutboundPlayerPacket>>,
    mut released: ResMut<Messages<crate::world::bus::OutboundPlayerReleased>>,
    sender: Res<FromDimSender<FromDim>>,
    mut backlog: Local<VecDeque<FromDim>>,
) {
    backlog.extend(msgs.drain().map(FromDim::Clientbound));
    backlog.extend(released.drain().map(|released| FromDim::Released {
        session: released.session,
    }));
    while let Some(outbound) = backlog.pop_front() {
        if let Err(flume::TrySendError::Full(outbound)) = sender.0.try_send(outbound) {
            backlog.push_front(outbound);
            break;
        }
    }
}

/// Marker component placed on the host-world entity that anchors a
/// `DimAppLabel`. The entity carries no other state — it exists purely to
/// allocate a `World`-unique ID for use as the sub-app label key.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

#[derive(Debug, thiserror::Error)]
#[error(
    "dimension {dimension} names dimension type {dimension_type}, which the loaded registry does not hold"
)]
pub struct UnknownDimensionType {
    pub dimension: String,
    pub dimension_type: u16,
}

/// Drain the `DimSpawnQueue` resource on the host world and materialise a
/// sub-app for each request. Called from outside the ECS run loop because
/// `App::insert_sub_app` requires `&mut App`.
pub fn drain_dim_spawn_queue(app: &mut App) {
    let requests: Vec<DimSpawnRequest> =
        std::mem::take(&mut app.world_mut().resource_mut::<DimSpawnQueue>().0);
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

/// Drain the `DimDespawnQueue` resource on the host world and tear down the
/// matching sub-apps. Called from outside the ECS run loop because
/// `App::remove_sub_app` requires `&mut App`.
pub fn drain_dim_despawn_queue(app: &mut App) {
    let entities: Vec<Entity> =
        std::mem::take(&mut app.world_mut().resource_mut::<DimDespawnQueue>().0);
    for entity in entities {
        if app.remove_sub_app(DimAppLabel(entity)).is_none() {
            warn!(
                ?entity,
                "DimDespawnQueue entry referenced a sub-app not registered under DimAppLabel"
            );
        }

        // Remove the channel entry for the despawned dim. After the sub-app
        // is gone its dim-side FromDimSender is dropped, so the host-side
        // receiver would return Disconnected on any subsequent drain attempt.
        // Removing the entry here keeps DimChannels consistent with the live
        // sub-app population and avoids holding a stale receiver.
        if let Some(mut channels) = app
            .world_mut()
            .get_resource_mut::<DimChannels<ToDim, FromDim>>()
        {
            channels.remove(entity);
        }
        // A dimension torn down saves none of its players, so nothing is left to wait for.
        crate::dim::forget_departures(app.world_mut(), |departing| departing.dim == entity);

        // Free the host-side label-anchor entity so the host world's
        // dimension-handle archetype matches the live sub-app population.
        // The OnRemove<DimSubAppHandle> observer fires before this drain runs
        // (it fires at despawn time), so when the observer path is active the
        // entity is already gone here — Err(_) is expected on that path.
        match app.world_mut().get_entity_mut(entity) {
            Ok(entity_mut) => entity_mut.despawn(),
            Err(_) => debug!(
                ?entity,
                "DimDespawnQueue entity already absent from host world (expected on the OnRemove observer path)"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::bus::{PacketPayload, PacketPriority, PacketTarget, TestPayload};
    use bevy_ecs::system::{IntoSystem, System};

    /// A tick that writes more than the channel holds must not cost a packet: a column is
    /// recorded as sent when it is queued and never offered again.
    #[test]
    fn a_full_channel_holds_the_overflow_back_instead_of_dropping_it() {
        let written = FROM_DIM_CAPACITY + FROM_DIM_CAPACITY / 2;
        let (tx, rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        let mut world = World::new();
        world.insert_resource(Messages::<OutboundPlayerPacket>::default());
        world.insert_resource(Messages::<crate::world::bus::OutboundPlayerReleased>::default());
        world.insert_resource(FromDimSender(tx));
        let mut msgs = world.resource_mut::<Messages<OutboundPlayerPacket>>();
        for seq in 0..written as u32 {
            msgs.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(Entity::PLACEHOLDER),
                priority: PacketPriority::Critical,
                data: PacketPayload::Test(TestPayload { seq }),
                session: mcrs_minecraft_level::session::PlayerSession(0),
                epoch: 0,
            });
        }

        let mut flush = IntoSystem::into_system(flush_from_dim_outbox);
        flush.initialize(&mut world);
        let mut arrived = Vec::new();
        let drain = |arrived: &mut Vec<u32>| {
            arrived.extend(rx.try_iter().map(|msg| match msg {
                FromDim::Clientbound(OutboundPlayerPacket {
                    data: PacketPayload::Test(payload),
                    ..
                }) => payload.seq,
                other => panic!("unexpected message {other:?}"),
            }));
        };

        flush.run((), &mut world).unwrap();
        drain(&mut arrived);
        assert_eq!(
            arrived.len(),
            FROM_DIM_CAPACITY,
            "the channel takes what it holds"
        );

        flush.run((), &mut world).unwrap();
        drain(&mut arrived);
        assert_eq!(
            arrived,
            (0..written as u32).collect::<Vec<_>>(),
            "every packet arrives, in the order it was written"
        );
    }
}
