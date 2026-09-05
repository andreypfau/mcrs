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
use tracing::{debug, warn};

use crate::world::bus::{
    InboundConfirmMove, InboundEntitySpawn, InboundPlayerDespawn, InboundPlayerSpawn,
    InboundRollbackMove, OutboundPlayerPacket,
};
use crate::world::channel_types::{FromDim, ToDim};
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_voxel_world::world::channels::{
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
use crate::WorldSave;
use crate::world::aoi::PlayerTrackerPlugin;
use crate::world::block::MinecraftBlockPlugin;
use crate::world::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use crate::world::format::anvil::SavedColumns;
use crate::world::light::DimLightPlugin;
use crate::world::loot::LootPlugin;
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_core::registry::access::RegistryAccess;
use mcrs_minecraft_core::registry::static_registry::StaticRegistry;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::block::Block;
use mcrs_minecraft_world::block::definition::Blocks;
use mcrs_minecraft_world::enchantment::EnchantmentData;
use mcrs_minecraft_world::worldgen::beta_biome::ActiveBiomeSource;
use mcrs_minecraft_worldgen::bevy::WorldGenConfig;
use mcrs_voxel_world::world::dimension::{DimensionBundle, DimensionPlugin, HasSkyLight};
use mcrs_voxel_world::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};

#[derive(Clone)]
pub struct DimRegistryBundle {
    pub registry_access: RegistryAccess,
    pub light_registry: Option<std::sync::Arc<mcrs_minecraft_light::block::LightRegistry>>,
    pub blocks: Blocks,
    pub static_enchantment_registry: StaticRegistry<EnchantmentData>,
    pub block_tag_registry: DynTagRegistry<Block>,
    pub biome_registry: RegistrySnapshot<Biome>,
    pub active_biome_source: Option<ActiveBiomeSource>,
    pub world_save: Option<WorldSave>,
    pub world_gen_config: WorldGenConfig,
}

pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        registry_access: world.resource::<RegistryAccess>().clone(),
        light_registry: world
            .get_resource::<crate::block_light_table::BlockLightRegistry>()
            .map(|registry| registry.0.clone()),
        blocks: world.resource::<Blocks>().clone(),
        static_enchantment_registry: world.resource::<StaticRegistry<EnchantmentData>>().clone(),
        block_tag_registry: world.resource::<DynTagRegistry<Block>>().clone(),
        biome_registry: world.resource::<RegistrySnapshot<Biome>>().clone(),
        active_biome_source: world.get_resource::<ActiveBiomeSource>().cloned(),
        world_save: world.get_resource::<WorldSave>().cloned(),
        world_gen_config: world
            .get_resource::<WorldGenConfig>()
            .cloned()
            .unwrap_or_else(WorldGenConfig::from_env),
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
) -> Entity {
    let label_entity = app
        .world_mut()
        .spawn((
            DimSubAppHandle,
            DimLabel(request.dimension_id.as_str().to_string()),
        ))
        .id();

    let (to_dim_srv_tx, to_dim_srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (to_dim_ctl_tx, to_dim_ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_dim_tx, from_dim_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);

    app.world_mut()
        .resource_mut::<DimChannels<ToDim, FromDim>>()
        .insert(
            label_entity,
            mcrs_voxel_world::world::channels::DimSender::new(to_dim_srv_tx),
            mcrs_voxel_world::world::channels::DimSender::new(to_dim_ctl_tx),
            from_dim_rx,
        );

    let asset_root = app
        .world()
        .get_resource::<mcrs_voxel_server::AssetRoot>()
        .cloned()
        .unwrap_or_default()
        .0;

    let mut sub_app = SubApp::new();

    sub_app.insert_resource(ToDimReceiver::<ToDim> {
        serverbound: to_dim_srv_rx,
        control: to_dim_ctl_rx,
    });
    sub_app.insert_resource(FromDimSender::<FromDim>(
        mcrs_voxel_world::world::channels::DimSender::new(from_dim_tx),
    ));

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
    // `InboundPlayerPacket` is read by `dispatch_inbound_to_dim` (added by
    // `MinecraftEntityPlugin`). Serverbound packets arrive via `drain_to_dim_inbox`.
    sub_app.add_message::<crate::world::bus::InboundPlayerPacket>();
    // `OutboundPlayerAttached` is written by `consume_inbound_player_spawn` and
    // extracted by the host to set `in_dim_entity` on the session entry.
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

    sub_app.init_resource::<mcrs_voxel_world::session::DimPlayerIndex>();

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
    let dim_for_tick = request.dimension_id.0.clone();
    #[cfg(feature = "telemetry-tracy")]
    let dim_for_extract = request.dimension_id.0.clone();
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
    sub_app.add_systems(
        ColumnDrain,
        (
            crate::world::chunk::process_completed_columns,
            // The light packet walks the column index, which is rebuilt here rather than left
            // to the tick: a column sent before its sections are in it goes out unlit.
            mcrs_voxel_world::world::storage::column::reconcile_column_existence,
            mcrs_voxel_world::world::storage::column::reconcile_column_chunks,
            mcrs_voxel_world::entity::player::chunk_view::update_loading_queue,
            crate::world::entity::player::column_view::load_chunk_request,
            crate::world::entity::player::column_view::load_column_queue,
            mcrs_voxel_world::world::lifecycle::ticket::spawn_chunks,
            crate::world::chunk::enqueue_pending_columns,
            crate::world::chunk::dispatch_column_generation.run_if(
                bevy_ecs::prelude::resource_exists::<
                    mcrs_minecraft_worldgen::bevy::OverworldNoiseRouter,
                >,
            ),
            crate::world::entity::player::column_view::loading_column_queue,
            crate::world::entity::player::column_view::send_column_queue,
            flush_from_dim_outbox,
        )
            .chain(),
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
    sub_app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        file_path: asset_root,
        ..AssetPlugin::default()
    });
    // The worldgen `ChunkPlugin` (NoiseGeneratorSettings, ColumnScheduler, the
    // CHUNK_TASK_POOL, and the five FixedPreUpdate worldgen systems) is the
    // per-dim entry-point that turns DimSpawnRequest into populated columns.
    // It is distinct from the engine-level `storage::chunk::ChunkPlugin` that
    // DimensionPlugin adds (which only contributes TicketPlugin).
    sub_app.insert_resource(registries.world_gen_config.clone());
    sub_app.add_plugins(crate::world::chunk::ChunkPlugin);
    // Per-dim composition of the simulation plugins. Each plugin's
    // schedule placements (`MinecraftBlockPlugin`, `ExplosionPlugin`,
    // `PlayerTrackerPlugin`, `BlockUpdatePlugin`, `BlockUpdateWirePlugin`,
    // `MinecraftEntityPlugin`, `LootPlugin`) run inside the per-dim
    // sub-app's `World`. `ExplosionPlugin::tick_explode` writes
    // `MessageWriter<BlockSetRequest>`; `BlockUpdatePlugin`'s
    // `apply_voxel_set_requests` reads the same per-dim buffer in the
    // same world, restoring the single-hop block-update path. The
    // additional `BlockUpdateWirePlugin` (defined in
    // `crate::world::block_update`) registers the per-dim wire-emit
    // system that fans block updates out via `OutboundPlayerPacket`
    // through `Column.PlayerObservers`. `MinecraftBlockPlugin` carries
    // the per-block TNT sub-plugin which reads
    // `MessageReader<PlayerWillDestroyBlock>` — the host-side
    // `digging.rs` writers route a clone of each event into
    // `PendingInboundLifecycle.block_events`, drained per-dim by the
    // extract closure below.
    sub_app.add_plugins(MinecraftBlockPlugin);
    sub_app.add_plugins(ExplosionPlugin);
    sub_app.add_plugins(PlayerTrackerPlugin);
    sub_app.add_plugins(BlockUpdatePlugin::default());
    sub_app.add_plugins(BlockUpdateWirePlugin);
    sub_app.add_plugins(MinecraftEntityPlugin);
    sub_app.add_plugins(LootPlugin);
    sub_app.add_plugins(crate::world::experience::ExperiencePlugin);
    sub_app.add_plugins(crate::world::arrival::ArrivalPlugin);
    if let Some(registry) = &registries.light_registry {
        sub_app.add_plugins(DimLightPlugin {
            registry: std::sync::Arc::clone(registry),
            bounds: mcrs_minecraft_light::level::LightBounds::from_dimension(
                request.type_config.min_y,
                request.type_config.section_count,
            ),
            sky: request.has_sky,
        });
    } else if !crate::lighting_disabled() {
        warn!(
            dim = request.dimension_id.as_str(),
            "no block light table; this dimension will publish no light"
        );
    }

    sub_app.insert_resource(registries.registry_access.clone());
    sub_app.insert_resource(registries.blocks.clone());
    sub_app.insert_resource(registries.static_enchantment_registry.clone());
    sub_app.insert_resource(registries.block_tag_registry.clone());
    sub_app.insert_resource(registries.biome_registry.clone());
    if let Some(active_biome_source) = &registries.active_biome_source {
        sub_app.insert_resource(active_biome_source.clone());
    }
    if let Some(world_save) = &registries.world_save
        && let Some(saved) = SavedColumns::open(&world_save.0, request.dimension_id.as_str())
    {
        sub_app.insert_resource(saved);
    }

    // Seed the time resources so an inspector that reads `Res<Time<…>>` on a
    // sub-app that has never been pumped gets a valid default. The extract
    // closure overwrites these every subsequent tick.
    sub_app.insert_resource(Time::<Fixed>::default());
    sub_app.insert_resource(Time::<Virtual>::default());
    sub_app.insert_resource(Time::<Real>::default());
    sub_app.init_resource::<mcrs_minecraft_world::world_clock::WorldClocks>();

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
        mcrs_minecraft_world::world_clock::extract_world_clocks(main_world, sub_world);

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

    // Resolve this dimension's index in the dimension_type registry that is
    // sent to the client during configuration. The client uses this index to
    // pick the DimensionType (and thus the chunk-section count) for its
    // ClientLevel, so the play-login emitter must send the matching value.
    // Vanilla dimensions use a dimension key equal to their type ident.
    let type_ident = request.dimension_id.as_str();
    let dim_type_index = registries
        .registry_access
        .iter()
        .find(|r| r.registry_key() == "minecraft:dimension_type")
        .and_then(|reg| {
            reg.iter_entries()
                .position(|e| e.location.as_str() == type_ident)
        })
        .map(|i| i as i32)
        .unwrap_or(0);

    let dim_entity = sub_app
        .world_mut()
        .spawn((
            DimensionBundle::new(request.dimension_id.clone(), request.type_config),
            DimTypeIndex(dim_type_index),
        ))
        .id();
    if request.has_sky {
        sub_app
            .world_mut()
            .entity_mut(dim_entity)
            .insert(HasSkyLight);
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
    label_entity
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
            ToDim::Spawn {
                host_anchor,
                session,
                snapshot,
                dimensions,
            } => {
                spawn_msgs.write(InboundPlayerSpawn {
                    host_anchor,
                    session,
                    snapshot,
                    dimensions,
                });
            }
            ToDim::Despawn {
                host_anchor,
                session,
            } => {
                despawn_msgs.write(InboundPlayerDespawn {
                    host_anchor,
                    session,
                });
            }
            ToDim::Serverbound { .. } => {}
            ToDim::SpawnEntity {
                move_id,
                epoch,
                cause,
                payload,
                player,
            } => {
                entity_spawn_msgs.write(InboundEntitySpawn {
                    move_id,
                    epoch,
                    cause,
                    payload,
                    player,
                });
            }
            ToDim::ConfirmMove { move_id } => {
                confirm_msgs.write(InboundConfirmMove { move_id });
            }
            ToDim::RollbackMove { move_id } => {
                rollback_msgs.write(InboundRollbackMove { move_id });
            }
        }
    }
    for msg in rx.serverbound.try_iter() {
        if let ToDim::Serverbound {
            player,
            id,
            data,
            timestamp,
        } = msg
        {
            serverbound_msgs.write(crate::world::bus::InboundPlayerPacket {
                player,
                id,
                data,
                timestamp,
            });
        }
    }
}

/// Emit a `FromDim`-channel-full warning on the first drop and then once every
/// this many further drops, so a saturated channel is visible in logs without
/// flooding them.
const FROM_DIM_DROP_LOG_INTERVAL: u64 = 256;

/// Drains the dim's local `Messages<OutboundPlayerPacket>` into the `FromDim`
/// channel so the host-side `pump_channels` can epoch-stamp and forward them.
///
/// A full channel means the host is not draining fast enough; the packet is
/// dropped (clientbound packet loss is recoverable like network loss), but the
/// loss is counted in `FROM_DIM_CHANNEL_DROP_TOTAL` and surfaced via a
/// rate-limited warning so saturation is not silently invisible.
pub(crate) fn flush_from_dim_outbox(
    mut msgs: ResMut<Messages<OutboundPlayerPacket>>,
    sender: Res<FromDimSender<FromDim>>,
    mut dropped_since_log: Local<u64>,
) {
    use mcrs_voxel_world::session::PlayerSession;
    use std::sync::atomic::Ordering;
    for msg in msgs.drain() {
        let outbound = FromDim::Clientbound {
            target: msg.target,
            priority: msg.priority,
            data: msg.data,
            session: PlayerSession(0),
            epoch: 0,
        };
        if sender.0.try_send(outbound).is_err() {
            mcrs_minecraft_network::metrics::FROM_DIM_CHANNEL_DROP_TOTAL
                .fetch_add(1, Ordering::Relaxed);
            let total = mcrs_minecraft_network::metrics::FROM_DIM_CHANNEL_DROP_TOTAL
                .load(Ordering::Relaxed);
            *dropped_since_log += 1;
            if *dropped_since_log == 1
                || dropped_since_log.is_multiple_of(FROM_DIM_DROP_LOG_INTERVAL)
            {
                warn!(
                    target: "mcrs_minecraft_server::bridge",
                    from_dim_drop_total = total,
                    "FromDim channel full; dropping clientbound packet (host not draining)"
                );
            }
        }
    }
}

/// Marker component placed on the host-world entity that anchors a
/// `DimAppLabel`. The entity carries no other state — it exists purely to
/// allocate a `World`-unique ID for use as the sub-app label key.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

/// The dimension-type registry index for a dimension's type, resolved
/// host-side from `RegistryAccess` when the sub-app is spawned and stored on
/// the sub-world's `Dimension` entity. The play-login emitter copies it into
/// `PlayerSpawnInfo.dimension_type_id` so a real client builds its
/// `ClientLevel` with the correct height (section count).
#[derive(bevy_ecs::component::Component, Clone, Copy)]
pub struct DimTypeIndex(pub i32);

/// The dimension resource location (e.g. "minecraft:the_nether") of the
/// sub-app anchored by this host-world label entity. Lets a name-based
/// transfer request resolve to the destination sub-app's label entity.
#[derive(bevy_ecs::component::Component, Clone)]
pub struct DimLabel(pub String);

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
        spawn_dim_subapp(app, &request, &bundle);
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
