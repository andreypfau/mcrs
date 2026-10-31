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
use std::collections::VecDeque;
use tracing::{error, warn};

use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerPacket};
use mcrs_minecraft_engine::channel_types::{FromDim, ToDim};
use mcrs_minecraft_engine::dim::DimSubAppHandle;
use mcrs_minecraft_level::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_minecraft_level::world::channels::{
    DimChannels, FROM_DIM_CAPACITY, FromDimSender, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    ToDimReceiver,
};

/// System set for inbox drain systems, run early in `FixedPreUpdate`.
/// Systems that consume inbound messages run after this set.
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
///
/// It has no `StateTransition` or `TimePlugin` either. Its time resources are copies of the
/// host's. `Res<Time>` is the fixed time in the `Fixed*` schedules and the virtual time in the
/// others. The `Fixed*` schedules run once per host tick, but the host can advance `Time<Fixed>`
/// by zero, one or more steps in that tick.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

use crate::world::aoi::PlayerTrackerPlugin;
use crate::world::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use crate::world::entity::MinecraftEntityPlugin;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironment;
use mcrs_minecraft_engine::heightmap::{BlockHeightmapPredicates, DimHeightmapPlugin};
use mcrs_minecraft_heightmap::HeightmapPredicates;
use mcrs_minecraft_level::world::column_drain::{ColumnDrain, ColumnDrainSet};
use mcrs_minecraft_level::world::dimension::{
    DimensionBundle, DimensionPlugin, DimensionTypeConfig, DimensionTypeId, HasSkyLight, HasWeather,
};
use mcrs_minecraft_level::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_level::world::lifecycle::trace::{ColumnTraceLog, ColumnTraceSink};
use mcrs_minecraft_level::world::sub_app::{
    DimAppLabel, DimSpawnQueue, DimSpawnRequest, DimensionPlugins,
};
use mcrs_minecraft_level::world::world_save::WorldSave;
use mcrs_minecraft_light_plugin::DimLightPlugin;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Clone)]
pub struct DimRegistryBundle {
    pub light_registry: Option<std::sync::Arc<mcrs_minecraft_light::block::LightRegistry>>,
    pub heightmap_predicates: Option<HeightmapPredicates>,
    pub world_save: Option<WorldSave>,
}

pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        light_registry: world
            .get_resource::<mcrs_minecraft_light::block_light::BlockLightRegistry>()
            .map(|registry| registry.0.clone()),
        heightmap_predicates: world
            .get_resource::<BlockHeightmapPredicates>()
            .map(|predicates| predicates.0.clone()),
        world_save: world.get_resource::<WorldSave>().cloned(),
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
#[expect(
    clippy::too_many_lines,
    reason = "a dimension's sub-app is assembled in the order its schedules, extraction and plugins require, and each step relies on the previous one"
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
                let id =
                    mcrs_minecraft_core::Id::checked(request.dimension_type.number(), types.len())?;
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

    let column_traces = app.world().get_resource::<ColumnTraceSink>().cloned();
    let trace_dimension = request.dimension.as_str().to_owned();
    let mut sub_app = SubApp::new();
    mcrs_minecraft_world::registry_snapshot::extract_registries(app.world(), sub_app.world_mut());

    sub_app.insert_resource(ToDimReceiver::<ToDim> {
        serverbound: to_dim_srv_rx,
        control: to_dim_ctl_rx,
    });
    sub_app.insert_resource(FromDimSender::<FromDim>(from_dim_tx));
    sub_app.init_resource::<FromDimBacklog>();

    // Per-sub-app message registrations. Only types that still flow through
    // the dim sub-app's Messages<T> double-buffer (intra-dim use) are kept.
    // Types that previously crossed the host↔dim boundary via the extract
    // closure are now carried by the ToDim/FromDim flume channels instead
    // and must NOT be registered here: nothing in the dimension writes or reads
    // a stray registration, and it hides which side owns the message.
    sub_app.add_message::<OutboundPlayerPacket>();
    // `InboundPlayerSpawn` is written by `drain_to_dim_inbox` from `ToDim::Spawn`
    // and read by `consume_inbound_player_spawn` (added by `MinecraftEntityPlugin`).
    sub_app.add_message::<InboundPlayerSpawn>();
    // `InboundPlayerDespawn` is written by `drain_to_dim_inbox` from `ToDim::Despawn`
    // and read by `drain_inbound_player_despawn` (added by `PlayerTrackerPlugin`).
    sub_app.add_message::<mcrs_minecraft_engine::bus::InboundPlayerDespawn>();
    // Written by `despawn_inbound_player` once the player is saved and drained into the
    // `FromDim` channel by `flush_from_dim_outbox`.
    sub_app.add_message::<mcrs_minecraft_engine::bus::OutboundPlayerReleased>();
    // `InboundPlayerPacket` is read by `dispatch_inbound_to_dim` (added by
    // `MinecraftEntityPlugin`). Serverbound packets arrive via `drain_to_dim_inbox`.
    sub_app.add_message::<mcrs_minecraft_engine::bus::InboundPlayerPacket>();
    // `OutboundPlayerAttached` is written by `consume_inbound_player_spawn` and
    // extracted by the host to attach the session.
    sub_app.add_message::<mcrs_minecraft_engine::bus::OutboundPlayerAttached>();
    sub_app.add_message::<mcrs_minecraft_level::entity::player::ChangeDimensionRequest>();
    // The sub-app builder is the single source of truth for these registrations — `BlockUpdatePlugin`
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
    sub_app.insert_resource(mcrs_minecraft_level::world::sub_app::DimensionLabel(
        label_entity,
    ));

    sub_app.update_schedule = Some(DimTick.intern());
    // `init_schedule` keeps a schedule that `SubApp::new` already filled, such as `First` with
    // `message_update_system`.
    sub_app.init_schedule(DimTick);
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
            let fixed = world.resource::<Time<Fixed>>().as_generic();
            world.insert_resource(fixed);
            world.run_schedule(FixedFirst);
            world.run_schedule(FixedPreUpdate);
            world.run_schedule(FixedUpdate);
            world.run_schedule(FixedPostUpdate);
            world.run_schedule(FixedLast);
            let virtual_time = world.resource::<Time<Virtual>>().as_generic();
            world.insert_resource(virtual_time);
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
            // The light packet walks the column index, which is rebuilt here rather than left
            // to the tick: a column sent before its sections are in it goes out unlit.
            (
                mcrs_minecraft_level::world::storage::column::reconcile_columns,
                mcrs_minecraft_level::world::storage::block_entity::reconcile_block_entities,
                mcrs_minecraft_engine::heightmap::prime_column_heightmaps,
            )
                .chain()
                .in_set(ColumnDrainSet::Reconcile),
            (
                mcrs_minecraft_level::world::lifecycle::ticket::propagate_section_levels,
                mcrs_minecraft_level::world::lifecycle::ticket::spawn_chunks,
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
            mcrs_minecraft_assets::packs::layered_file_source(
                &roots,
                mcrs_minecraft_worldgen_builtin::asset,
            ),
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
    if let Some(registry) = &registries.light_registry {
        sub_app.add_plugins(DimLightPlugin {
            registry: std::sync::Arc::clone(registry),
            bounds: mcrs_minecraft_light::level::LightBounds::from_dimension(
                type_config.min_y,
                type_config.section_count,
            ),
            sky: has_sky,
        });
    }

    if let Some(predicates) = &registries.heightmap_predicates {
        sub_app.insert_resource(BlockHeightmapPredicates(predicates.clone()));
    }
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
    sub_app.init_resource::<mcrs_minecraft_engine::clock::GameTime>();
    sub_app.init_resource::<crate::ops::OpList>();

    sub_app.set_extract(move |main_world, sub_world| {
        use mcrs_minecraft_engine::bus::OutboundPlayerAttached;

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
        mcrs_minecraft_engine::clock::extract_game_time(main_world, sub_world);
        mcrs_minecraft_world::registry_snapshot::extract_registries(main_world, sub_world);
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

    // Before the dimension entity spawns, so a plugin can still register required components and
    // hooks for its components and its observers see it spawn.
    if let Some(builders) = app.world().get_resource::<DimensionBuilders>() {
        let build = DimensionBuild {
            dimension: &request.dimension,
            type_config,
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

    app.insert_sub_app(DimAppLabel(label_entity), sub_app);
    Ok(label_entity)
}

/// Drains both `ToDim` channel receivers into the dim's local message buses.
/// The control channel (Spawn/Despawn) is drained first so lifecycle messages
/// always precede any same-tick Serverbound packet for a freshly spawned player.
fn drain_to_dim_inbox(
    rx: Res<ToDimReceiver<ToDim>>,
    mut spawn_msgs: ResMut<Messages<InboundPlayerSpawn>>,
    mut despawn_msgs: ResMut<Messages<InboundPlayerDespawn>>,
    mut serverbound_msgs: ResMut<Messages<mcrs_minecraft_engine::bus::InboundPlayerPacket>>,
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
        }
    }
    for msg in rx.serverbound.try_iter() {
        if let ToDim::Serverbound(m) = msg {
            serverbound_msgs.write(m);
        }
    }
}

/// What a dimension produced and its full channel did not take yet, oldest first.
#[derive(bevy_ecs::resource::Resource, Default)]
pub(crate) struct FromDimBacklog(pub(crate) VecDeque<FromDim>);

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
    mut released: ResMut<Messages<mcrs_minecraft_engine::bus::OutboundPlayerReleased>>,
    sender: Res<FromDimSender<FromDim>>,
    mut backlog: ResMut<FromDimBacklog>,
) {
    let backlog = &mut backlog.0;
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

/// Drain the `DimSpawnQueue` resource on the host world and materialise a
/// sub-app for each request. Called from outside the ECS run loop because
/// `App::insert_sub_app` requires `&mut App`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::{IntoSystem, System};
    use mcrs_minecraft_engine::bus::{PacketPayload, PacketPriority, PacketTarget, TestPayload};

    /// A tick that writes more than the channel holds must not cost a packet: a column is
    /// recorded as sent when it is queued and never offered again.
    #[test]
    fn a_full_channel_holds_the_overflow_back_instead_of_dropping_it() {
        let written = FROM_DIM_CAPACITY + FROM_DIM_CAPACITY / 2;
        let (tx, rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        let mut world = World::new();
        world.insert_resource(Messages::<OutboundPlayerPacket>::default());
        world.insert_resource(
            Messages::<mcrs_minecraft_engine::bus::OutboundPlayerReleased>::default(),
        );
        world.insert_resource(FromDimSender(tx));
        world.init_resource::<FromDimBacklog>();
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
