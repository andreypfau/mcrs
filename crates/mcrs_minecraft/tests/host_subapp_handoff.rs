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
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_engine::session::PlayerSession;
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::bridge::{bridge_inbound_to_channel, bridge_player_attach};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PlayerTransferSnapshot,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_engine::session::{PlayerSessionCounter, SessionRegistry};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PendingInboundBuffer, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_spawn_queue, DimSubAppHandle};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_protocol::uuid::Uuid;
use mcrs_vanilla::biome::Biome;
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
    app.insert_resource(RegistrySnapshot::<Biome>::default());

    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<PendingInboundBuffer>();
    app.init_resource::<DimChannelsResource>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(
        Update,
        (bridge_inbound_to_channel, bridge_player_attach),
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
/// entity exists (with a registered channel), the host must send exactly one
/// `ToDim::Spawn` into the dim's control channel and set
/// `SessionEntry.dim` to that label (no longer Entity::PLACEHOLDER).
#[test]
fn game_transition_emits_initial_spawn() {
    use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
    use mcrs_minecraft::world::channel_types::FromDim;

    let mut app = build_host_app();

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);

    // Spawn a fake live DimSubAppHandle label entity on the host and register
    // its channel so emit_initial_player_spawn can look it up.
    let dim_label = app.world_mut().spawn(DimSubAppHandle).id();
    let ctl_rx = {
        let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        app.world_mut()
            .resource_mut::<DimChannelsResource>()
            .insert(dim_label, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
        ctl_rx
    };

    // Transition to Game — the emit system should pick this up
    transition_to_game(&mut app, connection_entity);
    app.update();

    let spawns: Vec<ToDim> = ctl_rx
        .try_iter()
        .filter(|m| matches!(m, ToDim::Spawn { .. }))
        .collect();
    assert_eq!(
        spawns.len(),
        1,
        "exactly one ToDim::Spawn should be sent to the dim's control channel"
    );
    match &spawns[0] {
        ToDim::Spawn { host_anchor: ha, .. } => {
            assert_eq!(*ha, host_anchor, "spawn's host_anchor must match");
        }
        _ => unreachable!(),
    }

    let world = app.world();
    let (_, entry) = world
        .resource::<SessionRegistry>()
        .get_by_anchor(&host_anchor)
        .expect("SessionEntry present");
    assert_eq!(
        entry.dim, dim_label,
        "SessionEntry.dim must be set to the selected dim label (not PLACEHOLDER)"
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
    // No live dim → no channel found → emit_initial_player_spawn returned early.
    // The session dim should still be PLACEHOLDER.
    assert!(
        world.resource::<DimChannelsResource>().iter().count() == 0,
        "no channel should be registered when no DimSubAppHandle is live"
    );

    let (_, entry) = world
        .resource::<SessionRegistry>()
        .get_by_anchor(&host_anchor)
        .expect("SessionEntry present");
    assert_eq!(
        entry.dim,
        Entity::PLACEHOLDER,
        "dim must remain PLACEHOLDER when no dim is live"
    );
}

/// A host-anchor that already has current_dim != PLACEHOLDER (initial join
/// already emitted) must not send a second ToDim::Spawn.
#[test]
fn idempotent_single_emit() {
    use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
    use mcrs_minecraft::world::channel_types::FromDim;

    let mut app = build_host_app();

    let (connection_entity, _host_anchor) = spawn_accepted_connection(&mut app);

    let dim_label = app.world_mut().spawn(DimSubAppHandle).id();

    // Register a channel so emit_initial_player_spawn can find it.
    let ctl_rx = {
        let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        app.world_mut()
            .resource_mut::<DimChannelsResource>()
            .insert(dim_label, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
        ctl_rx
    };

    // First Game transition — emits the spawn on the control channel
    transition_to_game(&mut app, connection_entity);
    app.update();

    // Drain the first spawn so the channel is empty again
    let first_spawns: Vec<_> = ctl_rx
        .try_iter()
        .filter(|m| matches!(m, ToDim::Spawn { .. }))
        .collect();
    assert_eq!(first_spawns.len(), 1, "exactly one spawn on first tick");

    // Tick again — should NOT emit a second spawn (current_dim is already set)
    app.update();

    let second_spawns = ctl_rx
        .try_iter()
        .filter(|m| matches!(m, ToDim::Spawn { .. }))
        .count();
    assert_eq!(
        second_spawns,
        0,
        "no second ToDim::Spawn after current_dim is already set"
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

    // Send a ToDim::Spawn on the dim's control channel so drain_to_dim_inbox
    // routes it to Messages<InboundPlayerSpawn> inside the sub-app.
    let host_anchor = app.world_mut().spawn_empty().id();
    {
        app
            .world()
            .resource::<DimChannelsResource>()
            .get(dim_label)
            .expect("channel registered by spawn_dim_subapp")
            .control_sender
            .try_send(ToDim::Spawn {
                host_anchor,
                session: PlayerSession(0),
                snapshot: PlayerTransferSnapshot {
                    uuid: Uuid::new_v4(),
                    username: "consumer_test".into(),
                    position: DVec3::new(0.0, 64.0, 0.0),
                    rotation: Vec2::ZERO,
                },
            }).expect("control channel not full");
    }

    // Tick 1: drain_to_dim_inbox routes spawn to sub-app Messages<InboundPlayerSpawn>;
    // sub-app consumer spawns the entity + writes OutboundPlayerAttached.
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

    // Tick 1: emit_initial_player_spawn sends ToDim::Spawn on control channel →
    //         drain_to_dim_inbox routes it to sub-app Messages<InboundPlayerSpawn> →
    //         consume_inbound_player_spawn runs, spawns entity, writes
    //         OutboundPlayerAttached to sub-world buffer.
    app.update();

    // Tick 2: main bridge_player_attach runs (host buffer still empty) →
    //         extract drains sub-world OutboundPlayerAttached into host
    //         Messages<OutboundPlayerAttached>.
    app.update();

    // Tick 3: bridge_player_attach reads host Messages<OutboundPlayerAttached>
    //         → sets in_dim_entity.
    app.update();

    let (_, entry) = app
        .world()
        .resource::<SessionRegistry>()
        .get_by_anchor(&host_anchor)
        .expect("SessionEntry present");
    assert!(
        entry.in_dim_entity.is_some(),
        "SessionEntry.in_dim_entity must be Some after the full handoff round-trip"
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
        app
            .world()
            .resource::<DimChannelsResource>()
            .get(dim_label)
            .expect("channel registered by spawn_dim_subapp")
            .control_sender
            .try_send(ToDim::Spawn {
                host_anchor,
                session: PlayerSession(0),
                snapshot: PlayerTransferSnapshot {
                    uuid: Uuid::new_v4(),
                    username: "cursor_test".into(),
                    position: DVec3::new(0.0, 64.0, 0.0),
                    rotation: Vec2::ZERO,
                },
            }).expect("control channel not full");
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
