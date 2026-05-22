//! Validates the per-dim plugin migration:
//!
//! - `MinecraftBlockPlugin` and `ExplosionPlugin` run inside each
//!   per-dim sub-app, with the per-sub-app `Messages<T>` buffers their
//!   systems read and write registered.
//! - Cross-boundary `PlayerWillDestroyBlock` events routed through
//!   `PendingInboundLifecycle.block_events` reach exactly the
//!   destination sub-app and no other.
//! - `BlockUpdatePlugin`, `MinecraftEntityPlugin`, and `LootPlugin`
//!   remain host-side because their per-dim migration is blocked by
//!   host-side `ServerSideConnection` / per-dim entity ownership
//!   dependencies handled by later work.

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::{DVec3, Vec2};
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft::world::WorldPlugin;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_minecraft::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};
use mcrs_protocol::BlockStateId;
use mcrs_protocol::uuid::Uuid;
use smallvec::SmallVec;

mod harness {
    use bevy_app::App;
    use bevy_asset::AssetPlugin;
    use bevy_state::app::{AppExtStates, StatesPlugin};
    use bevy_state::prelude::NextState;
    use bevy_time::{Fixed, Time, TimePlugin};
    use mcrs_core::AppState;
    use mcrs_core::registry::access::RegistryAccess;
    use mcrs_core::registry::static_registry::StaticRegistry;
    use mcrs_core::tag::TagRegistry;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
    use mcrs_minecraft_lighting::table::BlockStateLightTable;
    use mcrs_vanilla::block::Block;
    use mcrs_vanilla::enchantment::EnchantmentData;

    pub fn make_stub_block_light_table() -> BlockStateLightTable {
        let state_count = 2usize;
        let emission = vec![0u8; state_count].into_boxed_slice();
        let dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let flags = vec![0u8; state_count].into_boxed_slice();
        BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    pub fn make_main_app_with_minimal_plugins() -> App {
        static SET_ASSET_ROOT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        SET_ASSET_ROOT.get_or_init(|| {
            let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .expect("CARGO_MANIFEST_DIR must have two ancestors (workspace root)");
            // SAFETY: set_var is safe before any thread reads the value;
            // the OnceLock guard guarantees a single mutation.
            unsafe {
                std::env::set_var("BEVY_ASSET_ROOT", workspace_root);
            }
        });

        let mut app = App::new();
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(TimePlugin);
        app.insert_resource(Time::<Fixed>::from_hz(20.0));
        app.add_plugins(StatesPlugin);
        app.init_state::<AppState>();
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();
        app.insert_resource(RegistryAccess::default());
        app.insert_resource(make_stub_block_light_table());
        app.insert_resource(StaticRegistry::<Block>::new());
        app.insert_resource(StaticRegistry::<EnchantmentData>::default());
        app.insert_resource(TagRegistry::<Block>::default());
        app
    }

    #[allow(dead_code)]
    pub fn drive_to_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        app.update();
    }

    pub fn enqueue_spawn(app: &mut App, id: &str, sky: bool) {
        app.world_mut()
            .resource_mut::<DimSpawnQueue>()
            .0
            .push(DimSpawnRequest {
                dimension_id: DimensionId::new(id),
                type_config: DimensionTypeConfig::default(),
                has_sky: sky,
            });
    }

    pub fn materialise_sub_apps(app: &mut App, ids: &[(&str, bool)]) {
        for (id, sky) in ids {
            enqueue_spawn(app, id, *sky);
        }
        drain_dim_spawn_queue(app);
    }
}

#[test]
fn minecraft_block_plugin_messages_present_in_each_subapp() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(
        &mut app,
        &[("test:overworld", true), ("test:nether", false)],
    );

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps expected");

    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let world = sub_app.world();

        // Invariant: per-sub-app registration for PlayerWillDestroyBlock
        // so the extract closure can write into the buffer without
        // panicking on resource_mut.
        assert!(
            world.contains_resource::<Messages<PlayerWillDestroyBlock>>(),
            "Messages<PlayerWillDestroyBlock> missing in sub-app {:?}",
            label
        );
        // Pre-existing OutboundPlayerPacket bus registration must still
        // be in place (defensive regression check).
        assert!(
            world.contains_resource::<Messages<OutboundPlayerPacket>>(),
            "Messages<OutboundPlayerPacket> missing in sub-app {:?}",
            label
        );
    }
}

#[test]
fn explosion_plugin_registered_per_dim_not_host() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app expected");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let world = sub_app.world();

    // ExplosionPlugin::tick_explode writes MessageWriter<BlockSetRequest>
    // per-dim. The buffer must exist so the system does not panic on
    // first use. BlockPlaced is registered for symmetry.
    assert!(
        world.contains_resource::<Messages<BlockSetRequest>>(),
        "Messages<BlockSetRequest> missing in per-dim world — \
         tick_explode would panic on MessageWriter<BlockSetRequest>"
    );
    assert!(
        world.contains_resource::<Messages<BlockPlaced>>(),
        "Messages<BlockPlaced> missing in per-dim world"
    );
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel(u8);

#[derive(Resource, Default)]
struct BlockEventLog(Vec<BlockPos>);

fn record_block_events(
    mut reader: MessageReader<PlayerWillDestroyBlock>,
    mut log: ResMut<BlockEventLog>,
) {
    for event in reader.read() {
        log.0.push(event.block_pos);
    }
}

#[test]
fn player_will_destroy_block_reaches_per_dim_consumer_via_lifecycle_partition() {
    let mut app = App::new();

    // Host-side: register the bus messages the extract closures read,
    // PlayerIndex, PendingInboundLifecycle.
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();

    // Allocate per-dim label entities from the host world so they are
    // distinct from any sub-app's internal allocator.
    let src_label_entity = app.world_mut().spawn_empty().id();
    let dest_label_entity = app.world_mut().spawn_empty().id();

    // Seed PlayerIndex so a synthetic host_anchor lives in the source dim.
    let host_anchor = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        host_anchor,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim: src_label_entity,
            previous_dim: None,
            in_dim_entity: Some(Entity::PLACEHOLDER),
            inbound_pending: SmallVec::new(),
        },
    );

    // Push directly into the source dim's block_events bucket. This
    // simulates the digging.rs writer's routing call — the test focuses
    // on the shuttle path, not the writer.
    app.world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(src_label_entity)
        .or_default()
        .block_events
        .push(PlayerWillDestroyBlock {
            player: Entity::PLACEHOLDER,
            chunk: Entity::PLACEHOLDER,
            block_pos: BlockPos::new(7, 64, 7),
            block_state: BlockStateId(0),
        });

    insert_test_sub_app(&mut app, TestDimLabel(0), src_label_entity);
    insert_test_sub_app(&mut app, TestDimLabel(1), dest_label_entity);

    app.update();

    let src_log = app
        .sub_app(TestDimLabel(0))
        .world()
        .resource::<BlockEventLog>()
        .0
        .clone();
    let dest_log = app
        .sub_app(TestDimLabel(1))
        .world()
        .resource::<BlockEventLog>()
        .0
        .clone();

    assert_eq!(
        src_log,
        vec![BlockPos::new(7, 64, 7)],
        "source sub-app must receive exactly one block event"
    );
    assert!(
        dest_log.is_empty(),
        "dest sub-app must not receive partition-isolated events"
    );
}

fn insert_test_sub_app(app: &mut App, label: TestDimLabel, label_entity: Entity) {
    let mut sub_app = SubApp::new();
    sub_app.update_schedule = Some(DimTick.intern());
    sub_app.add_schedule(Schedule::new(DimTick));
    sub_app.add_message::<PlayerWillDestroyBlock>();
    sub_app.init_resource::<BlockEventLog>();
    sub_app.add_systems(DimTick, record_block_events);

    sub_app.set_extract(move |main_world, sub_world| {
        let (_spawns, _despawns, block_events) = {
            let mut bundle = main_world.resource_mut::<PendingInboundLifecycle>();
            let entry = bundle.per_dim.entry(label_entity).or_default();
            (
                std::mem::take(&mut entry.spawns),
                std::mem::take(&mut entry.despawns),
                std::mem::take(&mut entry.block_events),
            )
        };
        if !block_events.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<PlayerWillDestroyBlock>>();
            for msg in block_events {
                sub_msgs.write(msg);
            }
        }
    });

    app.insert_sub_app(label, sub_app);
}

#[test]
fn host_side_no_longer_registers_per_dim_simulation_plugins() {
    // Build a host App with WorldPlugin so we can observe the host-side
    // registrations after the per-dim migration of BlockUpdatePlugin,
    // MinecraftEntityPlugin, and LootPlugin. We do not transition to
    // AppState::Playing; the assertions only inspect resources installed
    // during plugin build().
    let mut app = App::new();
    app.add_plugins(bevy_app::TaskPoolPlugin::default());
    app.add_plugins(bevy_asset::AssetPlugin::default());
    app.add_plugins(bevy_time::TimePlugin);
    app.add_plugins(bevy_state::app::StatesPlugin);
    use bevy_state::app::AppExtStates;
    app.init_state::<mcrs_core::AppState>();
    app.add_plugins(WorldPlugin);

    let world = app.world();

    // BlockUpdatePlugin is no longer host-side: its registrations of
    // Messages<BlockSetRequest> and Messages<BlockPlaced> must NOT be
    // present in the host world. They live in each per-dim sub-app
    // World instead.
    assert!(
        !world.contains_resource::<Messages<BlockSetRequest>>(),
        "BlockUpdatePlugin must no longer register Messages<BlockSetRequest> host-side"
    );
    assert!(
        !world.contains_resource::<Messages<BlockPlaced>>(),
        "BlockUpdatePlugin must no longer register Messages<BlockPlaced> host-side"
    );

    // MinecraftEntityPlugin is no longer host-side: its nested
    // PlayerPlugin -> PlayerActionPlugin chain must NOT have registered
    // Messages<PlayerAction> on the host. The chain runs per-dim.
    use mcrs_minecraft::world::entity::player::player_action::PlayerAction;
    assert!(
        !world.contains_resource::<Messages<PlayerAction>>(),
        "MinecraftEntityPlugin chain (PlayerActionPlugin) must no longer register \
         Messages<PlayerAction> host-side"
    );

    // LootPlugin is no longer host-side: BlockLootTables must NOT be
    // present in the host world.
    use mcrs_minecraft::world::loot::BlockLootTables;
    assert!(
        !world.contains_resource::<BlockLootTables>(),
        "LootPlugin must no longer install BlockLootTables host-side"
    );
}

#[test]
fn per_dim_simulation_plugins_now_in_sub_app() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app expected");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let world = sub_app.world();

    // BlockUpdatePlugin now per-dim: Messages<BlockSetRequest> and
    // Messages<BlockPlaced> live in the sub-app World.
    assert!(
        world.contains_resource::<Messages<BlockSetRequest>>(),
        "BlockUpdatePlugin must register Messages<BlockSetRequest> in the per-dim sub-app"
    );
    assert!(
        world.contains_resource::<Messages<BlockPlaced>>(),
        "BlockUpdatePlugin must register Messages<BlockPlaced> in the per-dim sub-app"
    );

    // MinecraftEntityPlugin now per-dim: Messages<PlayerAction> lives in
    // the sub-app World (via PlayerActionPlugin in the PlayerPlugin chain).
    use mcrs_minecraft::world::entity::player::player_action::PlayerAction;
    assert!(
        world.contains_resource::<Messages<PlayerAction>>(),
        "MinecraftEntityPlugin chain must register Messages<PlayerAction> in the per-dim sub-app"
    );

    // LootPlugin now per-dim: BlockLootTables lives in the sub-app World.
    use mcrs_minecraft::world::loot::BlockLootTables;
    assert!(
        world.contains_resource::<BlockLootTables>(),
        "LootPlugin must install BlockLootTables in the per-dim sub-app"
    );
}

fn _synthetic_snapshot_compile_only() {
    // Compile-only reference to keep PlayerTransferSnapshot reachable in
    // case future test additions need it without a re-import.
    let _ = mcrs_minecraft::world::bus::PlayerTransferSnapshot {
        uuid: Uuid::nil(),
        username: "x".into(),
        position: DVec3::ZERO,
        rotation: Vec2::ZERO,
    };
}
