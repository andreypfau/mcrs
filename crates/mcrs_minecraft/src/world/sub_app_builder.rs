use bevy_app::{
    App, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last,
    PluginsState, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, SubApp, Update,
};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_ecs::system::Local;
use bevy_ecs::world::World;
use bevy_time::{Fixed, Real, Time, Virtual};
use tracing::{debug, warn};

use crate::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    OutboundPlayerTransferRequest, PendingInboundLifecycle, PendingInboundPartition,
};
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};

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
use crate::world::aoi::PlayerTrackerPlugin;
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::{BlockUpdatePlugin, BlockUpdateWirePlugin};
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use crate::world::loot::LootPlugin;
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionPlugin, HasSkyLight,
};
use mcrs_engine::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::enchantment::EnchantmentData;
use mcrs_core::RegistrySnapshot;

#[derive(Clone)]
pub struct DimRegistryBundle {
    pub registry_access: RegistryAccess,
    pub block_light_table: BlockStateLightTable,
    pub static_block_registry: StaticRegistry<Block>,
    pub static_enchantment_registry: StaticRegistry<EnchantmentData>,
    pub block_tag_registry: TagRegistry<Block>,
    pub biome_registry: RegistrySnapshot<Biome>,
}

pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        registry_access: world.resource::<RegistryAccess>().clone(),
        block_light_table: world.resource::<BlockStateLightTable>().clone(),
        static_block_registry: world.resource::<StaticRegistry<Block>>().clone(),
        static_enchantment_registry: world.resource::<StaticRegistry<EnchantmentData>>().clone(),
        block_tag_registry: world.resource::<TagRegistry<Block>>().clone(),
        biome_registry: world.resource::<RegistrySnapshot<Biome>>().clone(),
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

    let mut sub_app = SubApp::new();

    // Per-sub-app bus message registration. Mirrors the host-side
    // `add_message::<T>()` block in `WorldPlugin::build`. The merged
    // extract closure below calls `resource_mut::<Messages<T>>()` on
    // BOTH worlds, so both must initialise the double-buffer before the
    // first pump.
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerPacket>();
    sub_app.add_message::<OutboundPlayerTransfer>();
    sub_app.add_message::<OutboundPlayerTransferRequest>();
    sub_app.add_message::<InboundPlayerSpawn>();
    sub_app.add_message::<OutboundPlayerAttached>();
    sub_app.add_message::<OutboundPlayerDisconnect>();
    sub_app.add_message::<InboundPlayerDespawn>();
    // Sub-side counterpart to the host-side `PlayerActionPlugin`
    // registration: the extract closure below shuttles
    // `PlayerWillDestroyBlock` from `PendingInboundLifecycle.block_events`
    // into the sub-world's buffer, and the per-dim TNT plugin reads it
    // via `MessageReader<PlayerWillDestroyBlock>`.
    sub_app.add_message::<PlayerWillDestroyBlock>();
    // `ExplosionPlugin::tick_explode` writes `MessageWriter<BlockSetRequest>`
    // and `BlockUpdatePlugin::apply_set_block_request` reads the same buffer;
    // both plugins now live in this per-dim sub-app so the explosion ->
    // block-set chain runs as a single message hop. The sub-app builder is
    // the single source of truth for these registrations — `BlockUpdatePlugin`
    // no longer registers them and instead debug-asserts they are already in
    // place, so a mistaken host-side `add_plugins(BlockUpdatePlugin)` fails
    // loud at plugin load rather than silently exporting the buffers to the
    // host world.
    sub_app.add_message::<BlockSetRequest>();
    sub_app.add_message::<BlockPlaced>();

    // `MinecraftEntityPlugin`'s nested `DiggingPlugin` and `PlayerPlugin`
    // carry systems that read host-side `PendingInboundLifecycle`,
    // `PlayerIndex`, and `LoadedWorldPreset` resources (the digging chain
    // routes per-player block events through the cross-`World` bridge;
    // `spawn_player` reads the loaded preset to fill in dimension state).
    // Now that the plugin runs per-dim, those systems live in this
    // sub-app's `World`, but their resource reads must not panic —
    // initialise empty per-dim copies so the systems no-op naturally
    // (the per-dim `PlayerIndex` will never contain entries because the
    // host owns the canonical index; the per-dim `PendingInboundLifecycle`
    // is never drained by an extract closure; `LoadedWorldPreset::is_loaded`
    // defaults to `false`, so `spawn_player` early-returns). The actual
    // cross-`World` event routing continues to flow through the host-side
    // copies of these resources.
    sub_app.init_resource::<crate::world::bus::PendingInboundLifecycle>();
    sub_app.init_resource::<crate::world::player_index::PlayerIndex>();
    sub_app.init_resource::<crate::configuration::LoadedWorldPreset>();

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

    sub_app.add_plugins(DimensionPlugin);
    sub_app.add_plugins(LightingPlugin);
    // AssetPlugin and AppTypeRegistry must precede any plugin that calls
    // `init_asset` / `register_asset_loader`. `ChunkPlugin` (via its nested
    // `NoiseGeneratorSettingsPlugin`) registers assets, so it must come after
    // this block. `AppTypeRegistry` is initialised by `App::new` but not by
    // `SubApp::new`, so the sub-app needs the explicit `init_resource` call.
    sub_app.init_resource::<bevy_ecs::reflect::AppTypeRegistry>();
    sub_app.add_plugins(AssetPlugin::default());
    // The worldgen `ChunkPlugin` (NoiseGeneratorSettings, ColumnScheduler, the
    // CHUNK_TASK_POOL, and the five FixedPreUpdate worldgen systems) is the
    // per-dim entry-point that turns DimSpawnRequest into populated columns.
    // It is distinct from the engine-level `storage::chunk::ChunkPlugin` that
    // DimensionPlugin adds (which only contributes TicketPlugin).
    sub_app.add_plugins(crate::world::chunk::ChunkPlugin);
    // Per-dim composition of the simulation plugins. Each plugin's
    // schedule placements (`MinecraftBlockPlugin`, `ExplosionPlugin`,
    // `PlayerTrackerPlugin`, `BlockUpdatePlugin`, `BlockUpdateWirePlugin`,
    // `MinecraftEntityPlugin`, `LootPlugin`) run inside the per-dim
    // sub-app's `World`. `ExplosionPlugin::tick_explode` writes
    // `MessageWriter<BlockSetRequest>`; `BlockUpdatePlugin`'s
    // `apply_set_block_request` reads the same per-dim buffer in the
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
    sub_app.add_plugins(BlockUpdatePlugin);
    sub_app.add_plugins(BlockUpdateWirePlugin);
    sub_app.add_plugins(MinecraftEntityPlugin);
    sub_app.add_plugins(LootPlugin);

    sub_app.insert_resource(registries.registry_access.clone());
    sub_app.insert_resource(registries.block_light_table.clone());
    sub_app.insert_resource(registries.static_block_registry.clone());
    sub_app.insert_resource(registries.static_enchantment_registry.clone());
    sub_app.insert_resource(registries.block_tag_registry.clone());
    sub_app.insert_resource(registries.biome_registry.clone());

    // Seed the time resources so an inspector that reads `Res<Time<…>>` on a
    // sub-app that has never been pumped gets a valid default. The extract
    // closure overwrites these every subsequent tick.
    sub_app.insert_resource(Time::<Fixed>::default());
    sub_app.insert_resource(Time::<Virtual>::default());
    sub_app.insert_resource(Time::<Real>::default());

    // Merged extract closure: time-resource shuttle (existing) + bus
    // shuttle (new). `SubApp::set_extract` replaces — does not compose —
    // so both directions of the bus and the time mirror live in one
    // closure. `label_entity` is captured by move; it is the host-world
    // Entity used as the `DimAppLabel` key AND the key in
    // `PendingInboundPartition.per_dim`. Capturing the in-sub-world
    // `dim_entity` (allocated further down) would silently break inbound
    // routing.
    sub_app.set_extract(move |main_world, sub_world| {
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

        let drained: Vec<OutboundPlayerPacket> = sub_world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect();
        if !drained.is_empty() {
            let mut main_msgs = main_world.resource_mut::<Messages<OutboundPlayerPacket>>();
            for msg in drained {
                main_msgs.write(msg);
            }
        }

        let inbound: Vec<InboundPlayerPacket> = main_world
            .resource_mut::<PendingInboundPartition>()
            .per_dim
            .entry(label_entity)
            .or_default()
            .drain(..)
            .collect();
        if !inbound.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<InboundPlayerPacket>>();
            for msg in inbound {
                sub_msgs.write(msg);
            }
        }

        let drained_transfers: Vec<OutboundPlayerTransfer> = sub_world
            .resource_mut::<Messages<OutboundPlayerTransfer>>()
            .drain()
            .collect();
        if !drained_transfers.is_empty() {
            let mut main_msgs = main_world.resource_mut::<Messages<OutboundPlayerTransfer>>();
            for msg in drained_transfers {
                main_msgs.write(msg);
            }
        }

        let drained_transfer_reqs: Vec<OutboundPlayerTransferRequest> = sub_world
            .resource_mut::<Messages<OutboundPlayerTransferRequest>>()
            .drain()
            .collect();
        if !drained_transfer_reqs.is_empty() {
            let mut main_msgs =
                main_world.resource_mut::<Messages<OutboundPlayerTransferRequest>>();
            for msg in drained_transfer_reqs {
                main_msgs.write(msg);
            }
        }

        let drained_attached: Vec<OutboundPlayerAttached> = sub_world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .drain()
            .collect();
        if !drained_attached.is_empty() {
            let mut main_msgs = main_world.resource_mut::<Messages<OutboundPlayerAttached>>();
            for msg in drained_attached {
                main_msgs.write(msg);
            }
        }

        let (spawns, despawns, block_events) = {
            let mut bundle = main_world.resource_mut::<PendingInboundLifecycle>();
            let entry = bundle.per_dim.entry(label_entity).or_default();
            (
                std::mem::take(&mut entry.spawns),
                std::mem::take(&mut entry.despawns),
                std::mem::take(&mut entry.block_events),
            )
        };
        if !spawns.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<InboundPlayerSpawn>>();
            for msg in spawns {
                sub_msgs.write(msg);
            }
        }
        if !despawns.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<InboundPlayerDespawn>>();
            for msg in despawns {
                sub_msgs.write(msg);
            }
        }
        if !block_events.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<PlayerWillDestroyBlock>>();
            for msg in block_events {
                sub_msgs.write(msg);
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
            DimensionBundle {
                dimension_id: request.dimension_id.clone(),
                type_config: request.type_config,
                ..Default::default()
            },
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
    let requests: Vec<DimSpawnRequest> = std::mem::take(
        &mut app.world_mut().resource_mut::<DimSpawnQueue>().0,
    );
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
    let entities: Vec<Entity> = std::mem::take(
        &mut app.world_mut().resource_mut::<DimDespawnQueue>().0,
    );
    for entity in entities {
        if app.remove_sub_app(DimAppLabel(entity)).is_none() {
            warn!(
                ?entity,
                "DimDespawnQueue entry referenced a sub-app not registered under DimAppLabel"
            );
        }

        // Purge host-side buckets keyed off this label_entity. Once the
        // sub-app is gone, no extract closure will ever drain
        // PendingInboundPartition.per_dim[entity] or
        // PendingInboundLifecycle.per_dim[entity], so any entries left
        // behind leak indefinitely. Removing the keys here keeps the
        // host maps consistent with the live sub-app population.
        if let Some(mut partition) =
            app.world_mut().get_resource_mut::<PendingInboundPartition>()
        {
            partition.per_dim.remove(&entity);
        }
        if let Some(mut lifecycle) =
            app.world_mut().get_resource_mut::<PendingInboundLifecycle>()
        {
            lifecycle.per_dim.remove(&entity);
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
