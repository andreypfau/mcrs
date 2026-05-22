use bevy_app::App;
use bevy_ecs::prelude::{Commands, Entity, ResMut};
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft::disconnect::process_disconnect;
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::bus::{InboundPlayerDespawn, PendingInboundLifecycle};
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex};
use mcrs_protocol::uuid::Uuid;

/// Construct a minimal `App` that wires `LoginPlugin` plus the
/// `PlayerIndex` resource and the `PendingInboundLifecycle` bucket the
/// disconnect cleanup routes despawn messages through. The two
/// `init_resource` calls duplicate what `WorldPlugin` registers in
/// production; they are inlined here so the test does not need to
/// spin up the entire host plugin stack.
fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundLifecycle>();
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
fn connection_removal_removes_player_index_entry_and_routes_despawn_via_lifecycle() {
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

    // Pin a concrete `current_dim` so the assertion can target a
    // specific bucket in `PendingInboundLifecycle.per_dim`. The login
    // observer seeds `current_dim` with `Entity::PLACEHOLDER`; pick a
    // non-placeholder value so the test fails loudly if the cleanup
    // ever stops honouring `current_dim`.
    let current_dim = Entity::from_raw_u32(77).expect("nonzero");
    app.world_mut()
        .resource_mut::<PlayerIndex>()
        .get_mut(&host_anchor)
        .expect("location present")
        .current_dim = current_dim;

    // Drive the cleanup helper directly because constructing a real
    // `ServerSideConnection` requires a `RawConnection` socket, which
    // is not accessible from an integration test. `process_disconnect`
    // is the canonical entry point used by `on_player_disconnect` (the
    // observer over `Remove, ServerSideConnection`) and by
    // `drain_pending_disconnects` (the deferred drain path); both
    // resolve `host_anchor` from a `HostAnchorRef` lookup before
    // delegating here. The observer-side resolution is exercised by the
    // login test above.
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut lifecycle: ResMut<PendingInboundLifecycle>| {
                process_disconnect(
                    host_anchor,
                    &mut player_index,
                    &mut lifecycle,
                    &mut commands,
                );

                // process_disconnect short-circuits on a missing index
                // entry (the `None` arm in its location lookup), so a
                // second call after the first removed the entry is a
                // no-op rather than a panic. Exercise that path too.
                process_disconnect(
                    host_anchor,
                    &mut player_index,
                    &mut lifecycle,
                    &mut commands,
                );
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

    let lifecycle = app.world().resource::<PendingInboundLifecycle>();
    let bundle = lifecycle
        .per_dim
        .get(&current_dim)
        .expect("lifecycle bucket for current_dim present");
    assert_eq!(
        bundle.despawns.len(),
        1,
        "exactly one despawn routed (second call short-circuits)",
    );
    let routed: InboundPlayerDespawn = InboundPlayerDespawn {
        host_anchor: bundle.despawns[0].host_anchor,
    };
    assert_eq!(routed.host_anchor, host_anchor);
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
