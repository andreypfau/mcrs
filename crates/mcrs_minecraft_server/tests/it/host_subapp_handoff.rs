//! Integration tests for the host→SubApp player-connection handoff on initial
//! join: the host-side emit, the per-dim consumer and the full round-trip.

use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, Vec2};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_assets::snapshot::RegistrySnapshot;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_item::Item;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::session::{Place, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registry::{Entries, Registry};
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft_server::world::bridge::{bridge_inbound_to_channel, bridge_player_attach};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PlayerTransferSnapshot,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::session::HostAnchorRef;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};

use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_server::configuration::emit_initial_player_spawn;

use crate::support;

/// Build a minimal host-side App with the bus substrate and the systems
/// under test.
fn build_host_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    support::insert_registries(&mut app);

    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_systems(Update, (bridge_inbound_to_channel, bridge_player_attach));

    app.add_plugins(LoginPlugin);
    // System under test
    app.add_systems(Update, emit_initial_player_spawn);

    app
}

/// Spawn a connection entity in `LoginState::Accepted` so `on_login_accepted`
/// fires and creates the host-anchor and its session.
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

/// Transition a connection entity to `ConnectionState::Game` — mirrors what
/// `on_configuration_ack` does.
fn transition_to_game(app: &mut App, connection_entity: Entity) {
    use mcrs_minecraft_network::ConnectionState;
    app.world_mut()
        .entity_mut(connection_entity)
        .insert(ConnectionState::Game);
}

// ---------------------------------------------------------------------------
// host-side emit_initial_player_spawn
// ---------------------------------------------------------------------------

/// When a connection transitions to Game AND a live DimSubAppHandle label
/// entity exists (with a registered channel), the host must send exactly one
/// `ToDim::Spawn` into the dim's control channel and mark the session as
/// joining that label.
fn game_transition_emits_initial_spawn() {
    use mcrs_minecraft_level::world::channels::{
        FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    };
    use mcrs_minecraft_server::world::channel_types::FromDim;

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
            .insert(dim_label, srv_tx, ctl_tx, from_rx);
        ctl_rx
    };

    // Transition to Game — the emit system should pick this up
    transition_to_game(&mut app, connection_entity);
    app.update();

    let spawns: Vec<ToDim> = ctl_rx
        .try_iter()
        .filter(|m| matches!(m, ToDim::Spawn(..)))
        .collect();
    assert_eq!(
        spawns.len(),
        1,
        "exactly one ToDim::Spawn should be sent to the dim's control channel"
    );
    match &spawns[0] {
        ToDim::Spawn(InboundPlayerSpawn {
            host_anchor: ha, ..
        }) => {
            assert_eq!(*ha, host_anchor, "spawn's host_anchor must match");
        }
        _ => unreachable!(),
    }

    let world = app.world();
    assert_eq!(
        world
            .get::<SessionPlacement>(host_anchor)
            .expect("session present")
            .place(),
        Place::Joining(dim_label),
        "the session must be joining the selected dim label"
    );
}

fn a_player_saved_in_another_dimension_joins_that_dimension() {
    use mcrs_minecraft_level::world::channels::{
        FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    };
    use mcrs_minecraft_server::WorldSave;
    use mcrs_minecraft_server::world::channel_types::FromDim;
    use mcrs_minecraft_server::world::sub_app_builder::DimLabel;
    use mcrs_minecraft_world::save::{PlayerDat, write_player_dat};

    let mut app = build_host_app();
    let save = std::env::temp_dir().join(format!("mcrs-join-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&save).unwrap();
    app.insert_resource(WorldSave(save.clone()));

    let (connection_entity, host_anchor) = spawn_accepted_connection(&mut app);
    let uuid = app
        .world()
        .get::<GameProfile>(connection_entity)
        .expect("profile")
        .id;
    write_player_dat(
        &save,
        uuid,
        &PlayerDat {
            dimension: "minecraft:the_nether".to_owned(),
            ..PlayerDat::default()
        },
    )
    .unwrap();

    let mut labels = Vec::new();
    for name in ["minecraft:overworld", "minecraft:the_nether"] {
        let label = app
            .world_mut()
            .spawn((DimSubAppHandle, DimLabel(name.to_owned())))
            .id();
        let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        app.world_mut()
            .resource_mut::<DimChannelsResource>()
            .insert(label, srv_tx, ctl_tx, from_rx);
        labels.push((label, ctl_rx));
    }

    transition_to_game(&mut app, connection_entity);
    app.update();

    let (nether, nether_rx) = &labels[1];
    assert_eq!(
        app.world()
            .get::<SessionPlacement>(host_anchor)
            .expect("session present")
            .place(),
        Place::Joining(*nether),
    );
    assert_eq!(nether_rx.try_iter().count(), 1);
    assert_eq!(labels[0].1.try_iter().count(), 0);
    std::fs::remove_dir_all(&save).unwrap();
}

/// When no live DimSubAppHandle label entity exists yet (dims still loading),
/// the emitter must NOT push any spawn and must leave the session unplaced.
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

    assert_eq!(
        world
            .get::<SessionPlacement>(host_anchor)
            .expect("session present")
            .place(),
        Place::Unplaced,
        "the session must stay unplaced when no dim is live"
    );
}

// ---------------------------------------------------------------------------
// per-dim consumer + full round-trip
// ---------------------------------------------------------------------------

/// MessageReader cursor semantics: a second pump with no new InboundPlayerSpawn
/// must NOT spawn a second in-dim entity.
fn no_duplicate_spawn_on_reread() {
    use mcrs_minecraft_level::entity::player::Player;
    use mcrs_minecraft_level::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_minecraft_level::world::sub_app::DimAppLabel;

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
            type_config: DimensionTypeConfig::new(-64, 384),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);

    let dim_label = {
        let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
        q.iter(app.world())
            .map(|(e, _)| e)
            .next()
            .expect("one DimSubAppHandle")
    };

    let host_anchor = app.world_mut().spawn_empty().id();
    {
        app.world()
            .resource::<DimChannelsResource>()
            .get(dim_label)
            .expect("channel registered by spawn_dim_subapp")
            .control_sender
            .try_send(ToDim::Spawn(InboundPlayerSpawn {
                host_anchor,
                session: PlayerSession(0),
                snapshot: PlayerTransferSnapshot {
                    uuid: Uuid::new_v4(),
                    username: "cursor_test".into(),
                    position: DVec3::new(0.0, 64.0, 0.0),
                    rotation: Vec2::ZERO,
                    view_distance: 12,
                },
                dimensions: Vec::new(),
            }))
            .expect("control channel not full");
    }

    // Tick 1: consumer reads the spawn and materializes one entity
    app.update();
    // Tick 2 and 3: no new spawn — cursor must not re-read
    app.update();
    app.update();

    let player_count = {
        let sub = app.sub_app_mut(DimAppLabel(dim_label));
        let world = sub.world_mut();
        world
            .query_filtered::<Entity, With<Player>>()
            .iter(world)
            .count()
    };
    assert_eq!(
        player_count, 1,
        "cursor semantics: only one Player entity despite multiple pumps after a single spawn"
    );
}

#[test]
fn entering_the_game_spawns_the_player_once_in_its_saved_dimension() {
    game_transition_emits_initial_spawn();
    a_player_saved_in_another_dimension_joins_that_dimension();
    no_live_dim_no_spawn();
    no_duplicate_spawn_on_reread();
}

#[test]
fn every_shared_registry_reaches_every_dimension_as_the_hosts_arc() {
    use mcrs_minecraft_block::definition::Blocks;
    use mcrs_minecraft_item::Items;
    use mcrs_minecraft_registry::RegistrySet;
    use mcrs_minecraft_registry::shared::SharedRegistries;
    use mcrs_minecraft_world::entity::minecraft::EntityIds;
    use std::any::type_name;

    let expected = [
        type_name::<RegistrySet>(),
        type_name::<EntityIds>(),
        type_name::<RegistryAccess>(),
        type_name::<Blocks>(),
        type_name::<Items>(),
        type_name::<Registry<EnchantmentData>>(),
        type_name::<Entries<EnchantmentData, EnchantmentData>>(),
        type_name::<DynTagRegistry<Block>>(),
        type_name::<DynTagRegistry<Item>>(),
        type_name::<RegistrySnapshot<Biome>>(),
    ];

    let mut app = crate::host_app::make_host_app();
    crate::host_app::materialise_sub_apps(
        &mut app,
        &[("test:overworld", true), ("test:nether", false)],
    );

    let shared = app.world().resource::<SharedRegistries>();
    let dimensions: Vec<_> = app.sub_apps().sub_apps.values().collect();
    assert_eq!(dimensions.len(), 2);
    for dimension in dimensions {
        let seen = shared.shared_in(app.world(), dimension.world());
        for name in expected {
            assert!(
                seen.iter().any(|(seen, _)| *seen == name),
                "{name}: {seen:?}"
            );
        }
        for (name, state) in seen {
            assert_eq!(state, Some(true), "{name}");
        }
    }
}
