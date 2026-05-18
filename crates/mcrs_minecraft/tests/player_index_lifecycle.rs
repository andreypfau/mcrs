use bevy_app::App;
use bevy_ecs::message::{MessageWriter, Messages};
use bevy_ecs::prelude::{Commands, Entity, ResMut};
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::bus::InboundPlayerDespawn;
use mcrs_minecraft::world::entity::player::cleanup_host_anchor;
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex};
use mcrs_protocol::uuid::Uuid;

/// Construct a minimal `App` that wires `LoginPlugin` plus the
/// `PlayerIndex` resource and the `InboundPlayerDespawn` message buffer.
///
/// The two `init_resource`/`add_message` calls duplicate what
/// `WorldPlugin` registers in production. They are inlined here so the
/// test does not need to spin up the entire host plugin stack.
fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<PlayerIndex>();
    app.add_message::<InboundPlayerDespawn>();
    app
}

fn insert_accepted_login(app: &mut App, profile: GameProfile) -> Entity {
    let entity = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(entity)
        .insert((profile, LoginState::Accepted));
    app.update();
    entity
}

fn fresh_profile() -> GameProfile {
    GameProfile {
        id: Uuid::new_v4(),
        username: "test_player".into(),
        properties: Vec::new(),
    }
}

#[test]
fn login_accepted_inserts_player_index_entry_and_host_anchor_ref() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());

    let world = app.world();

    assert_eq!(
        world.resource::<PlayerIndex>().len(),
        1,
        "one PlayerIndex entry after accepted login",
    );

    let host_anchor_ref = world
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("connection entity carries HostAnchorRef after login");

    assert!(
        world.get_entity(host_anchor_ref.0).is_ok(),
        "host-anchor entity exists in the world",
    );

    let location = world
        .resource::<PlayerIndex>()
        .get(&host_anchor_ref.0)
        .expect("PlayerLocation present for host-anchor");

    assert_eq!(location.socket, connection_entity);
    assert_eq!(location.current_dim, Entity::PLACEHOLDER);
    assert!(location.in_dim_entity.is_none());
    assert!(location.inbound_pending.is_empty());
}

#[test]
fn connection_removal_removes_player_index_entry_and_emits_despawn_message() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());

    let host_anchor = app
        .world()
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("login observer attached HostAnchorRef")
        .0;

    assert_eq!(app.world().resource::<PlayerIndex>().len(), 1);

    // Drive the cleanup helper directly because constructing a real
    // `ServerSideConnection` requires a `RawConnection` socket, which
    // is not accessible from an integration test. The full system
    // (`on_player_disconnect_cleanup_host_anchor`) only adds the
    // `RemovedComponents` loop on top of this helper; that loop has no
    // behavior beyond resolving each connection entity's
    // `HostAnchorRef`, which we already exercise via the login test.
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut despawn_writer: MessageWriter<InboundPlayerDespawn>| {
                let ran = cleanup_host_anchor(
                    &mut commands,
                    host_anchor,
                    &mut player_index,
                    &mut despawn_writer,
                );
                assert!(ran, "cleanup ran on first call");

                let ran_twice = cleanup_host_anchor(
                    &mut commands,
                    host_anchor,
                    &mut player_index,
                    &mut despawn_writer,
                );
                assert!(!ran_twice, "second call is a no-op (idempotent)");
            },
        )
        .expect("system runs without panicking");

    // Run an extra frame so `Commands::despawn` is applied before we
    // assert against the world.
    app.update();

    assert!(
        app.world().resource::<PlayerIndex>().is_empty(),
        "PlayerIndex emptied after cleanup",
    );

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "host-anchor entity despawned after cleanup",
    );

    let despawn_messages: Vec<Entity> = app
        .world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .drain()
        .map(|m| m.host_anchor)
        .collect();

    assert_eq!(
        despawn_messages.len(),
        1,
        "exactly one InboundPlayerDespawn emitted (second call short-circuits)",
    );
    assert_eq!(despawn_messages[0], host_anchor);
}

#[test]
fn login_observer_no_ops_for_non_accepted_login_states() {
    let mut app = make_app();
    let entity = app.world_mut().spawn_empty().id();

    // Insert a non-Accepted LoginState alongside a profile. The
    // observer fires on every Add<LoginState> but must filter out
    // anything other than `Accepted`.
    app.world_mut()
        .entity_mut(entity)
        .insert((fresh_profile(), LoginState::Hello));
    app.update();

    assert!(
        app.world().resource::<PlayerIndex>().is_empty(),
        "no PlayerIndex entry for non-Accepted login state",
    );
    assert!(
        app.world().entity(entity).get::<HostAnchorRef>().is_none(),
        "no HostAnchorRef attached for non-Accepted login state",
    );
}
