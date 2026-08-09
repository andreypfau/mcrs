use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::system::RunSystemOnce;
use mcrs_engine::session::{PlayerSession, SessionRegistry};
use mcrs_minecraft::disconnect::process_disconnect;
use mcrs_minecraft::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft::world::bus::InboundPlayerDespawn;
use mcrs_minecraft::world::channel_types::DimChannelsResource;
use mcrs_minecraft::world::player_index::{HostAnchorRef, PlayerIndex};
use mcrs_protocol::uuid::Uuid;

fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<mcrs_engine::session::PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
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
fn login_accepted_inserts_session_registry_entry_and_host_anchor_ref() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());

    let world = app.world();

    let session_registry = world.resource::<SessionRegistry>();
    assert_eq!(
        session_registry.iter().count(),
        1,
        "one SessionRegistry entry after accepted login",
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

    let (_, entry) = session_registry
        .get_by_anchor(&host_anchor_ref.0)
        .expect("SessionEntry present for host-anchor");

    assert_eq!(entry.connection_entity, connection_entity);
    assert_eq!(entry.dim, Entity::PLACEHOLDER);
    assert!(entry.in_dim_entity.is_none());

    let player_index = world.resource::<PlayerIndex>();
    assert_eq!(player_index.len(), 1, "one PlayerIndex entry for the username");
    let session_from_index = player_index.get_by_username("test_player");
    assert!(session_from_index.is_some(), "username maps to a session");
    assert_ne!(
        session_from_index.unwrap(),
        PlayerSession(0),
        "session must not be the zero sentinel"
    );
}

#[test]
fn connection_removal_removes_session_entry_and_routes_despawn_via_lifecycle() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());

    let host_anchor = app
        .world()
        .entity(connection_entity)
        .get::<HostAnchorRef>()
        .copied()
        .expect("login observer attached HostAnchorRef")
        .0;

    assert_eq!(app.world().resource::<SessionRegistry>().iter().count(), 1);

    // Pin a concrete dim so the assertion can target a specific channel.
    let current_dim = Entity::from_raw_u32(77).expect("nonzero");
    let ctl_rx = {
        use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
        use mcrs_minecraft::world::channel_types::FromDim;
        let (srv_tx, _srv_rx) = flume::bounded::<mcrs_minecraft::world::channel_types::ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<mcrs_minecraft::world::channel_types::ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        app.world_mut()
            .resource_mut::<DimChannelsResource>()
            .insert(current_dim, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
        ctl_rx
    };
    let session = app
        .world()
        .resource::<SessionRegistry>()
        .get_by_anchor(&host_anchor)
        .map(|(s, _)| *s)
        .expect("session present");
    app.world_mut()
        .resource_mut::<SessionRegistry>()
        .get_mut(&session)
        .expect("entry present")
        .dim = current_dim;

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut session_registry: ResMut<SessionRegistry>,
                  dim_channels: ResMut<DimChannelsResource>| {
                process_disconnect(
                    host_anchor,
                    &mut player_index,
                    &mut session_registry,
                    &dim_channels,
                    &mut mcrs_engine::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );

                // Second call is a no-op (session already removed).
                process_disconnect(
                    host_anchor,
                    &mut player_index,
                    &mut session_registry,
                    &dim_channels,
                    &mut mcrs_engine::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );
            },
        )
        .expect("system runs without panicking");

    app.update();

    assert!(
        app.world().resource::<SessionRegistry>().iter().count() == 0,
        "SessionRegistry emptied after cleanup",
    );

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "host-anchor entity despawned after cleanup",
    );

    let despawn_msgs: Vec<_> = ctl_rx.try_iter().collect();
    assert_eq!(
        despawn_msgs.len(),
        1,
        "exactly one despawn routed to control channel (second call short-circuits)",
    );
    match &despawn_msgs[0] {
        mcrs_minecraft::world::channel_types::ToDim::Despawn {
            host_anchor: ha,
            session: sess,
        } => {
            assert_eq!(*ha, host_anchor);
            assert_eq!(
                *sess, session,
                "despawn must carry the real session so the dim can evict its DimPlayerIndex entry"
            );
        }
        other => panic!("expected ToDim::Despawn, got {other:?}"),
    }
}

#[test]
fn login_observer_no_ops_for_non_accepted_login_states() {
    let mut app = make_app();
    let entity = app.world_mut().spawn_empty().id();

    app.world_mut()
        .entity_mut(entity)
        .insert((fresh_profile(), LoginState::Hello));
    app.update();

    assert!(
        app.world().resource::<SessionRegistry>().iter().count() == 0,
        "no SessionRegistry entry for non-Accepted login state",
    );
    assert!(
        app.world().entity(entity).get::<HostAnchorRef>().is_none(),
        "no HostAnchorRef attached for non-Accepted login state",
    );
}
