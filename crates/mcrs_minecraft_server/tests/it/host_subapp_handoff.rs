//! Integration tests for the host→SubApp player-connection handoff on initial
//! join: the host-side emit, the per-dim consumer and the full round-trip.

use bevy_app::{App, InternedAppLabel, Update};
use bevy_ecs::change_detection::Tick;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use bevy_math::{DVec3, Vec2};
use bevy_state::prelude::NextState;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::session::{Place, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft_server::world::bridge::{bridge_inbound_to_channel, bridge_player_attach};
use mcrs_minecraft_server::world::bus::{InboundPlayerSpawn, PlayerTransferSnapshot};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::session::HostAnchorRef;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};

use mcrs_minecraft_server::configuration::emit_initial_player_spawn;

use crate::support;
use mcrs_minecraft_dimension::Dimension;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

fn row(failed: &mut Vec<String>, name: &str, check: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(check)) {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("panicked");
        failed.push(format!("{name}: {message}"));
    }
}

fn rows_hold(failed: Vec<String>) {
    assert!(
        failed.is_empty(),
        "failing rows, the first is the cause:\n{}",
        failed.join("\n")
    );
}

/// Build a minimal host-side App with the bus substrate and the systems
/// under test.
fn build_host_app() -> App {
    let mut app = crate::host_app::make_host_app();
    app.insert_resource(support::dimension_list_with_extra());

    app.init_resource::<PlayerSessionCounter>();
    app.add_message::<InboundPlayerSpawn>();
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

/// A live dimension label on the host with its channels registered; returns the label and the
/// receiving end of its control channel.
fn live_dimension(app: &mut App, key: ResourceKey<Dimension>) -> (Entity, flume::Receiver<ToDim>) {
    use mcrs_minecraft_level::world::channels::{
        FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    };
    use mcrs_minecraft_server::world::channel_types::FromDim;

    let label = app.world_mut().spawn((DimSubAppHandle, key)).id();
    let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(label, srv_tx, ctl_tx, from_rx);
    (label, ctl_rx)
}

// ---------------------------------------------------------------------------
// host-side emit_initial_player_spawn
// ---------------------------------------------------------------------------

/// When a connection transitions to Game AND a live DimSubAppHandle label
/// entity exists (with a registered channel), the host must send exactly one
/// `ToDim::Spawn` into the dim's control channel and mark the session as
/// joining that label.
fn game_transition_emits_initial_spawn(
    app: &mut App,
    dim_label: Entity,
    ctl_rx: &flume::Receiver<ToDim>,
) {
    let (connection_entity, host_anchor) = spawn_accepted_connection(app);

    // Transition to Game — the emit system should pick this up
    transition_to_game(app, connection_entity);
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

/// Live dimensions are registered nether first, so a join that takes the first
/// live one lands in the wrong world.
fn join_with_live_nether_end_and_overworld(
    saved: Option<ResourceKey<Dimension>>,
    expected: ResourceKey<Dimension>,
) {
    use mcrs_minecraft_server::WorldSave;
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
    if let Some(dimension) = &saved {
        write_player_dat(
            &save,
            uuid,
            &PlayerDat {
                dimension: dimension.clone(),
                ..PlayerDat::default()
            },
        )
        .unwrap();
    }

    let live: Vec<_> = [
        mcrs_minecraft_dimension::keys::dimension::THE_NETHER,
        mcrs_minecraft_dimension::keys::dimension::THE_END,
        mcrs_minecraft_dimension::keys::dimension::OVERWORLD,
    ]
    .into_iter()
    .map(|name| {
        let key = ResourceKey::<Dimension>::from(name);
        let (label, control) = live_dimension(&mut app, key.clone());
        (key, label, control)
    })
    .collect();

    transition_to_game(&mut app, connection_entity);
    app.update();

    let joined = live
        .iter()
        .find(|(key, ..)| *key == expected)
        .expect("the expected dimension is live")
        .1;
    assert_eq!(
        app.world()
            .get::<SessionPlacement>(host_anchor)
            .expect("session present")
            .place(),
        Place::Joining(joined),
        "{saved:?}"
    );
    for (key, _, control) in &live {
        let spawns = usize::from(*key == expected);
        assert_eq!(control.try_iter().count(), spawns, "{saved:?} into {key:?}");
    }
    std::fs::remove_dir_all(&save).unwrap();
}

#[test]
fn a_joining_player_enters_its_saved_dimension_or_else_the_overworld() {
    let nether =
        ResourceKey::<Dimension>::from(mcrs_minecraft_dimension::keys::dimension::THE_NETHER);
    let overworld =
        ResourceKey::<Dimension>::from(mcrs_minecraft_dimension::keys::dimension::OVERWORLD);
    let gone = ResourceKey::from_location(ResourceLocation::read("test:gone").unwrap());
    for (saved, expected) in [
        (Some(nether.clone()), nether),
        (Some(gone), overworld.clone()),
        (None, overworld),
    ] {
        join_with_live_nether_end_and_overworld(saved, expected);
    }
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
            dimension: mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::keys::dimension_type::OVERWORLD.as_str(),
            ),
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
                dimensions: Vec::new().into(),
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

fn joining_players_share_the_baked_dimension_list(
    app: &mut App,
    ctl_rx: &flume::Receiver<ToDim>,
    baked: &Arc<[ResourceKey<Dimension>]>,
) {
    for _ in 0..2 {
        let (connection, _) = spawn_accepted_connection(app);
        transition_to_game(app, connection);
    }
    app.update();

    let lists: Vec<_> = ctl_rx
        .try_iter()
        .filter_map(|message| match message {
            ToDim::Spawn(spawn) => Some(spawn.dimensions),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 2);
    for sent in &lists {
        assert!(Arc::ptr_eq(sent, baked));
    }
}

#[test]
fn entering_the_game_spawns_the_player_once_and_shares_the_baked_dimension_list() {
    let mut failed = Vec::new();
    row(
        &mut failed,
        "no live dimension sends no spawn",
        no_live_dim_no_spawn,
    );
    row(
        &mut failed,
        "a re-read does not spawn a second player",
        no_duplicate_spawn_on_reread,
    );

    let mut app = build_host_app();
    let list = support::dimension_list_with_extra();
    let baked = Arc::clone(list.keys());
    app.insert_resource(list);
    let (dim_label, ctl_rx) = live_dimension(
        &mut app,
        mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
    );
    row(&mut failed, "the transition sends one spawn", || {
        game_transition_emits_initial_spawn(&mut app, dim_label, &ctl_rx)
    });
    row(
        &mut failed,
        "joining players share the baked dimension list",
        || joining_players_share_the_baked_dimension_list(&mut app, &ctl_rx, &baked),
    );
    rows_hold(failed);
}

fn every_registry_resource_of_the_host_reaches_every_dimension(app: &App) {
    use mcrs_minecraft_block::definition::Blocks;
    use mcrs_minecraft_core::Versioned;
    use mcrs_minecraft_item::Items;
    use mcrs_minecraft_registries::CurrentRegistries;
    use mcrs_minecraft_world::registry_snapshot::RegistrySnapshot;
    use std::any::type_name;

    let expected = [
        type_name::<CurrentRegistries>(),
        type_name::<Blocks>(),
        type_name::<Items>(),
    ];

    let resource_names = |world: &World| -> Vec<String> {
        world
            .iter_resources()
            .map(|(info, _)| info.name().to_string())
            .collect()
    };
    let candidates: Vec<String> = resource_names(app.world())
        .into_iter()
        .filter(|name| is_registry_type(name))
        .collect();
    assert!(
        candidates
            .iter()
            .any(|name| name.contains("CurrentRegistries")),
        "the host holds no registry resource to check: {candidates:?}"
    );

    let host_snapshot = app.world().resource::<Versioned<RegistrySnapshot>>();
    let dimensions: Vec<_> = app.sub_apps().sub_apps.values().collect();
    assert_eq!(dimensions.len(), 2);
    for dimension in dimensions {
        let held = resource_names(dimension.world());
        for name in expected {
            assert!(held.iter().any(|held| held == name), "{name}: {held:?}");
        }
        let missing: Vec<&String> = candidates
            .iter()
            .filter(|candidate| !held.contains(candidate))
            .collect();
        assert!(
            missing.is_empty(),
            "host resources of a registry type that no dimension received: {missing:#?}"
        );
        let snapshot = dimension.world().resource::<Versioned<RegistrySnapshot>>();
        assert!(
            Arc::ptr_eq(host_snapshot.value(), snapshot.value()),
            "a dimension holds its own snapshot, not the host's"
        );
    }
}

fn is_registry_type(name: &str) -> bool {
    let head = name.split('<').next().unwrap_or(name);
    let leaf = head.rsplit("::").next().unwrap_or(head);
    matches!(
        leaf,
        "Registry" | "Registries" | "CurrentRegistries" | "Blocks" | "Items"
    )
}

#[test]
fn every_registry_resource_of_the_host_reaches_every_dimension_through_one_snapshot() {
    let mut app = crate::host_app::make_host_app();
    crate::host_app::drive_to_playing(&mut app);
    crate::host_app::materialise_sub_apps(&mut app, &[OVERWORLD, NETHER]);

    let mut failed = Vec::new();
    row(
        &mut failed,
        "every registry resource of the host reaches every dimension",
        || every_registry_resource_of_the_host_reaches_every_dimension(&app),
    );
    row(
        &mut failed,
        "a registry resource is not changed on the second tick",
        || a_registry_resource_is_not_changed_on_the_second_tick(&mut app),
    );
    rows_hold(failed);
}

const OVERWORLD: (&str, &str) = ("minecraft:overworld", "minecraft:overworld");
const NETHER: (&str, &str) = ("minecraft:the_nether", "minecraft:the_nether");

fn dimension_labels(app: &mut App) -> Vec<(Entity, ResourceKey<Dimension>)> {
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &ResourceKey<Dimension>), With<DimSubAppHandle>>();
    query
        .iter(app.world())
        .map(|(label, key)| (label, key.clone()))
        .collect()
}

fn a_registry_resource_is_not_changed_on_the_second_tick(app: &mut App) {
    use mcrs_minecraft_level::world::sub_app::DimAppLabel;

    let labels = dimension_labels(app);
    assert_eq!(labels.len(), 2);
    let overworld = labels
        .iter()
        .find(|(_, key)| *key == mcrs_minecraft_dimension::keys::dimension::OVERWORLD)
        .expect("the overworld is live")
        .0;

    let host_anchor = app.world_mut().spawn_empty().id();
    app.world()
        .resource::<DimChannelsResource>()
        .get(overworld)
        .expect("channel registered by spawn_dim_subapp")
        .control_sender
        .try_send(ToDim::Spawn(InboundPlayerSpawn {
            host_anchor,
            session: PlayerSession(0),
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "second_tick".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: Vec2::ZERO,
                view_distance: 12,
            },
            dimensions: Vec::new().into(),
        }))
        .expect("control channel not full");

    app.update();
    {
        use mcrs_minecraft_level::entity::player::Player;
        let world = app.sub_app_mut(DimAppLabel(overworld)).world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Player>>()
                .iter(world)
                .count(),
            1
        );
    }
    let since: Vec<_> = labels
        .iter()
        .map(|(label, _)| {
            let world = app.sub_app_mut(DimAppLabel(*label)).world_mut();
            (*label, world.increment_change_tick())
        })
        .collect();
    app.update();

    for (label, since) in since {
        let world = app.sub_app(DimAppLabel(label)).world();
        assert!(
            !registry_resources_changed_since(world, since),
            "a registry resource changed on a tick without a swap"
        );
    }
}

fn registry_resources_changed_since(world: &World, since: Tick) -> bool {
    use mcrs_minecraft_core::Versioned;
    use mcrs_minecraft_registries::CurrentRegistries;
    use mcrs_minecraft_world::registry_snapshot::RegistrySnapshot;

    let now = world.read_change_tick();
    let snapshot = world
        .get_resource_change_ticks::<Versioned<RegistrySnapshot>>()
        .expect("the dimension holds the snapshot");
    let current = world
        .get_resource_change_ticks::<CurrentRegistries>()
        .expect("the dimension holds the registries");
    snapshot.is_changed(since, now) || current.is_changed(since, now)
}

fn assert_every_dimension_holds_the_hosts_registries(app: &App) {
    use mcrs_minecraft_registries::CurrentRegistries;

    let host = app.world().resource::<CurrentRegistries>();
    let dimensions = &app.sub_apps().sub_apps;
    assert!(!dimensions.is_empty(), "the host has no dimensions");
    for dimension in dimensions.values() {
        let held = dimension.world().resource::<CurrentRegistries>();
        assert!(
            Arc::ptr_eq(host.arc(), held.arc()),
            "dimension holds its own copy of the registries, not the host's Arc"
        );
    }
}

#[test]
fn every_dimension_holds_the_hosts_registries_arc() {
    let mut app = crate::host_app::make_host_app();
    crate::host_app::drive_to_playing(&mut app);
    crate::host_app::materialise_sub_apps(&mut app, &[OVERWORLD, NETHER]);

    assert_every_dimension_holds_the_hosts_registries(&app);
}

#[test]
#[should_panic(expected = "dimension holds its own copy of the registries, not the host's Arc")]
fn a_dimension_holding_a_cloned_registries_fails_the_pointer_check() {
    use mcrs_minecraft_registries::{CurrentRegistries, Registries};

    let mut app = crate::host_app::make_host_app();
    crate::host_app::drive_to_playing(&mut app);
    crate::host_app::materialise_sub_apps(&mut app, &[OVERWORLD, NETHER]);

    let cloned = Registries::clone(app.world().resource::<CurrentRegistries>().arc());
    app.sub_apps_mut()
        .sub_apps
        .values_mut()
        .next()
        .expect("a dimension is live")
        .world_mut()
        .insert_resource(CurrentRegistries::new(Arc::new(cloned)));

    assert_every_dimension_holds_the_hosts_registries(&app);
}

fn snapshot_ticks(app: &mut App) -> Vec<(InternedAppLabel, Tick)> {
    app.sub_apps_mut()
        .sub_apps
        .iter_mut()
        .map(|(label, dimension)| (*label, dimension.world_mut().increment_change_tick()))
        .collect()
}

fn dimensions_that_replaced_their_snapshot(app: &App, since: &[(InternedAppLabel, Tick)]) -> usize {
    use mcrs_minecraft_core::Versioned;
    use mcrs_minecraft_world::registry_snapshot::RegistrySnapshot;

    since
        .iter()
        .filter(|(label, since)| {
            let world = app.sub_apps().sub_apps[label].world();
            world
                .get_resource_change_ticks::<Versioned<RegistrySnapshot>>()
                .is_some_and(|ticks| ticks.is_changed(*since, world.read_change_tick()))
        })
        .count()
}

#[test]
fn a_swapped_set_reaches_every_dimension_on_the_next_tick_and_an_idle_tick_replaces_nothing() {
    use mcrs_minecraft_registries::{CurrentRegistries, Registries};
    use mcrs_minecraft_world::registry_snapshot::replace_registries;

    let mut app = crate::host_app::make_host_app();
    crate::host_app::drive_to_playing(&mut app);
    crate::host_app::materialise_sub_apps(&mut app, &[OVERWORLD, NETHER]);
    let dimensions = app.sub_apps().sub_apps.len();

    let old = Arc::clone(app.world().resource::<CurrentRegistries>().arc());
    replace_registries(app.world_mut(), Registries::clone(&old));

    let since = snapshot_ticks(&mut app);
    app.update();

    assert_every_dimension_holds_the_hosts_registries(&app);
    for dimension in app.sub_apps().sub_apps.values() {
        let held = dimension.world().resource::<CurrentRegistries>();
        assert!(
            !Arc::ptr_eq(&old, held.arc()),
            "a dimension still holds the replaced registries"
        );
    }
    assert_eq!(
        dimensions_that_replaced_their_snapshot(&app, &since),
        dimensions
    );

    let since = snapshot_ticks(&mut app);
    app.update();
    assert_eq!(dimensions_that_replaced_their_snapshot(&app, &since), 0);
}
