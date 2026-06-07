//! Integration tests for the host→SubApp player-connection handoff on initial
//! join. Task 1 covers the host-side emit; Task 2 covers the per-dim consumer
//! and the full round-trip.

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, Vec2};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::bridge::{bridge_player_attach, partition_main_inbound};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition, PlayerTransferSnapshot,
};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_protocol::uuid::Uuid;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;

// System under test (Task 1) — must be pub in configuration.rs
use mcrs_minecraft::configuration::emit_initial_player_spawn;

fn make_stub_block_light_table() -> BlockStateLightTable {
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

/// Build a minimal host-side App with the bus substrate and the systems
/// under test.
fn build_host_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
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

    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(
        Update,
        (partition_main_inbound, bridge_player_attach),
    );

    app.add_plugins(LoginPlugin);
    // System under test
    app.add_systems(Update, emit_initial_player_spawn);

    app
}

/// Spawn a connection entity in `LoginState::Accepted` so `on_login_accepted`
/// fires and creates the host-anchor + PlayerIndex entry.
/// Returns (connection_entity, host_anchor).
fn spawn_accepted_connection(app: &mut App) -> (Entity, Entity) {
    let profile = GameProfile {
        id: Uuid::new_v4(),
        username: "test_player".into(),
        properties: vec![],
    };
    let connection_entity = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(connection_entity)
        .insert((profile, LoginState::Accepted));
    // Flush the on_login_accepted observer
    app.update();

    let host_anchor = app
        .world()
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("HostAnchorRef present after login")
        .0;
    (connection_entity, host_anchor)
}

/// Transition a connection entity to `ConnectionState::Game` and insert
/// `InGameConnectionState` — mirrors what `on_configuration_ack` does.
fn transition_to_game(app: &mut App, connection_entity: Entity) {
    use mcrs_network::{ConnectionState, InGameConnectionState};
    app.world_mut()
        .entity_mut(connection_entity)
        .insert((ConnectionState::Game, InGameConnectionState));
}

// ---------------------------------------------------------------------------
// Task 1 tests — host-side emit_initial_player_spawn
// ---------------------------------------------------------------------------

/// When a connection transitions to Game AND a live DimSubAppHandle label
/// entity exists, the host must push exactly one InboundPlayerSpawn into
/// PendingInboundLifecycle.per_dim[dim_label].spawns and set
/// PlayerLocation.current_dim to that label (no longer Entity::PLACEHOLDER).
#[test]
fn game_transition_emits_initial_spawn() {
    let mut app = build_host_app();

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);

    // Spawn a fake live DimSubAppHandle label entity on the host
    let dim_label = app.world_mut().spawn(DimSubAppHandle).id();

    // Transition to Game — the emit system should pick this up
    transition_to_game(&mut app, connection_entity);
    app.update();

    let world = app.world();
    let lifecycle = world.resource::<PendingInboundLifecycle>();
    let bundle = lifecycle
        .per_dim
        .get(&dim_label)
        .expect("PendingInboundLifecycle should have a bucket for dim_label");

    assert_eq!(
        bundle.spawns.len(),
        1,
        "exactly one InboundPlayerSpawn should be pushed into the lifecycle bucket"
    );
    assert_eq!(
        bundle.spawns[0].host_anchor,
        host_anchor,
        "the spawn's host_anchor should match the host-anchor entity"
    );

    let location = world
        .resource::<PlayerIndex>()
        .get(&host_anchor)
        .expect("PlayerLocation present");
    assert_eq!(
        location.current_dim, dim_label,
        "PlayerLocation.current_dim must be set to the selected dim label (not PLACEHOLDER)"
    );
}

/// When no live DimSubAppHandle label entity exists yet (dims still loading),
/// the emitter must NOT push any spawn and must leave current_dim as PLACEHOLDER.
#[test]
fn no_live_dim_no_spawn() {
    let mut app = build_host_app();

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);

    // No DimSubAppHandle spawned — dims still loading
    transition_to_game(&mut app, connection_entity);
    app.update();

    let world = app.world();
    let lifecycle = world.resource::<PendingInboundLifecycle>();
    assert!(
        lifecycle.per_dim.is_empty(),
        "no spawn should be pushed when no DimSubAppHandle is live"
    );

    let location = world
        .resource::<PlayerIndex>()
        .get(&host_anchor)
        .expect("PlayerLocation present");
    assert_eq!(
        location.current_dim,
        Entity::PLACEHOLDER,
        "current_dim must remain PLACEHOLDER when no dim is live"
    );
}

/// A host-anchor that already has current_dim != PLACEHOLDER (initial join
/// already emitted) must not emit a second InboundPlayerSpawn.
#[test]
fn idempotent_single_emit() {
    let mut app = build_host_app();

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);

    let dim_label = app.world_mut().spawn(DimSubAppHandle).id();

    // First Game transition — emits the spawn
    transition_to_game(&mut app, connection_entity);
    app.update();

    // Drain the spawns so the bucket is empty again
    app.world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(dim_label)
        .or_default()
        .spawns
        .clear();

    // Tick again — should NOT emit a second spawn
    app.update();

    let world = app.world();
    let lifecycle = world.resource::<PendingInboundLifecycle>();
    let count = lifecycle
        .per_dim
        .get(&dim_label)
        .map(|b| b.spawns.len())
        .unwrap_or(0);
    assert_eq!(
        count,
        0,
        "no second InboundPlayerSpawn after current_dim is already set"
    );
}

// ---------------------------------------------------------------------------
// Task 2 tests — per-dim consumer + full round-trip
// ---------------------------------------------------------------------------

/// The per-dim consume_inbound_player_spawn system must spawn exactly one
/// in-dim entity carrying the Player marker, and write one OutboundPlayerAttached
/// into the sub-world's Messages<OutboundPlayerAttached>.
#[test]
fn spawn_consumer_materializes_in_dim_entity() {
    use mcrs_engine::entity::player::Player;
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_engine::world::sub_app::DimAppLabel;

    let mut app = build_host_app();

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    let dim_label = {
        let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
        q.iter(app.world()).map(|(e, _)| e).next().expect("one DimSubAppHandle")
    };

    // Push an InboundPlayerSpawn directly into the lifecycle buffer.
    let host_anchor = app.world_mut().spawn_empty().id();
    {
        let snapshot = PlayerTransferSnapshot {
            uuid: Uuid::new_v4(),
            username: "consumer_test".into(),
            position: DVec3::new(0.0, 64.0, 0.0),
            rotation: Vec2::ZERO,
        };
        app.world_mut()
            .resource_mut::<PendingInboundLifecycle>()
            .per_dim
            .entry(dim_label)
            .or_default()
            .spawns
            .push(InboundPlayerSpawn { host_anchor, snapshot });
    }

    // Tick 1: extract shuttles the spawn into the sub-app; sub-app consumer
    // runs and spawns the entity + writes OutboundPlayerAttached.
    app.update();
    // Tick 2: extract drains OutboundPlayerAttached to host.
    app.update();

    let player_count = {
        let sub = app.sub_app_mut(DimAppLabel(dim_label));
        let world = sub.world_mut();
        world.query_filtered::<Entity, With<Player>>().iter(world).count()
    };
    assert_eq!(player_count, 1, "exactly one Player entity in the sub-app");
}

/// Full production-topology test: host emits InboundPlayerSpawn on Game
/// transition → extract shuttles it → sub-app consumer spawns entity →
/// OutboundPlayerAttached extracted → host bridge_player_attach sets
/// in_dim_entity. After the pump, PlayerIndex.in_dim_entity must be Some.
#[test]
fn attach_roundtrip_sets_in_dim_entity() {
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};

    let mut app = build_host_app();

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    // Transition to Game — emit_initial_player_spawn fires
    transition_to_game(&mut app, connection_entity);

    // Tick 1: emit runs, lifecycle filled, extract shuttles spawn in,
    //         sub-app consumer spawns entity + writes OutboundPlayerAttached,
    //         extract drains OutboundPlayerAttached to host Messages buffer.
    app.update();

    // Tick 2: host bridge_player_attach reads OutboundPlayerAttached → sets in_dim_entity.
    app.update();

    let location = app
        .world()
        .resource::<PlayerIndex>()
        .get(&host_anchor)
        .expect("PlayerLocation present");
    assert!(
        location.in_dim_entity.is_some(),
        "PlayerIndex.in_dim_entity must be Some after the full handoff round-trip"
    );
}

/// MessageReader cursor semantics: a second pump with no new InboundPlayerSpawn
/// must NOT spawn a second in-dim entity.
#[test]
fn no_duplicate_spawn_on_reread() {
    use mcrs_engine::entity::player::Player;
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_engine::world::sub_app::DimAppLabel;

    let mut app = build_host_app();

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    let dim_label = {
        let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
        q.iter(app.world()).map(|(e, _)| e).next().expect("one DimSubAppHandle")
    };

    let host_anchor = app.world_mut().spawn_empty().id();
    {
        let snapshot = PlayerTransferSnapshot {
            uuid: Uuid::new_v4(),
            username: "cursor_test".into(),
            position: DVec3::new(0.0, 64.0, 0.0),
            rotation: Vec2::ZERO,
        };
        app.world_mut()
            .resource_mut::<PendingInboundLifecycle>()
            .per_dim
            .entry(dim_label)
            .or_default()
            .spawns
            .push(InboundPlayerSpawn { host_anchor, snapshot });
    }

    // Tick 1: consumer reads the spawn and materializes one entity
    app.update();
    // Tick 2 and 3: no new spawn — cursor must not re-read
    app.update();
    app.update();

    let player_count = {
        let sub = app.sub_app_mut(DimAppLabel(dim_label));
        let world = sub.world_mut();
        world.query_filtered::<Entity, With<Player>>().iter(world).count()
    };
    assert_eq!(
        player_count,
        1,
        "cursor semantics: only one Player entity despite multiple pumps after a single spawn"
    );
}
