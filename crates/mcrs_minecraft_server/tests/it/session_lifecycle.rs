use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Res};
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_level::session::{Place, PlayerSession, Session, SessionPlacement};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::disconnect::{LeavingSessions, process_disconnect};
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState};
use mcrs_minecraft_server::world::bus::InboundPlayerDespawn;
use mcrs_minecraft_server::world::channel_types::DimChannelsResource;
use mcrs_minecraft_server::world::session::{HostAnchorRef, SessionConnection};

fn make_app() -> App {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
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

fn anchor_of(app: &App, connection: Entity) -> Entity {
    app.world()
        .entity(connection)
        .get::<HostAnchorRef>()
        .expect("login observer attached HostAnchorRef")
        .0
}

fn session_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Session>()
        .iter(app.world())
        .count()
}

fn sessions_named(app: &mut App, username: &str) -> Vec<PlayerSession> {
    app.world_mut()
        .query::<(&Session, &GameProfile)>()
        .iter(app.world())
        .filter(|(_, profile)| profile.username == username)
        .map(|(session, _)| session.0)
        .collect()
}

#[test]
fn login_accepted_spawns_an_unplaced_session_on_the_host_anchor() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());

    assert_eq!(
        session_count(&mut app),
        1,
        "one session after accepted login"
    );

    let host_anchor = anchor_of(&app, connection_entity);
    let anchor = app
        .world()
        .get_entity(host_anchor)
        .expect("host-anchor entity exists in the world");
    assert_eq!(
        anchor
            .get::<SessionConnection>()
            .expect("the anchor knows its connection")
            .entity(),
        connection_entity
    );
    assert_eq!(
        anchor
            .get::<SessionPlacement>()
            .map(SessionPlacement::place),
        Some(Place::Unplaced)
    );

    let named = sessions_named(&mut app, "test_player");
    assert_eq!(named.len(), 1, "the username names one session");
    assert_ne!(
        named[0],
        PlayerSession(0),
        "session must not be the zero sentinel"
    );
}

#[test]
fn connection_removal_despawns_the_session_and_routes_despawn_via_lifecycle() {
    let mut app = make_app();
    let connection_entity = insert_accepted_login(&mut app, fresh_profile());
    let host_anchor = anchor_of(&app, connection_entity);

    // Pin a concrete dim so the assertion can target a specific channel.
    let current_dim = Entity::from_raw_u32(77).expect("nonzero");
    let ctl_rx = {
        use mcrs_minecraft_level::world::channels::{
            FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
        };
        use mcrs_minecraft_server::world::channel_types::FromDim;
        let (srv_tx, _srv_rx) =
            flume::bounded::<mcrs_minecraft_server::world::channel_types::ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<mcrs_minecraft_server::world::channel_types::ToDim>(
            TO_DIM_CONTROL_CAPACITY,
        );
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        app.world_mut()
            .resource_mut::<DimChannelsResource>()
            .insert(
                current_dim,
                srv_tx,
                ctl_tx,
                from_rx,
            );
        ctl_rx
    };
    let session = app
        .world()
        .get::<Session>(host_anchor)
        .expect("session present")
        .0;
    app.world_mut()
        .get_mut::<SessionPlacement>(host_anchor)
        .expect("placement present")
        .set(Place::InDim(current_dim));

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut sessions: LeavingSessions,
                  dim_channels: Res<DimChannelsResource>| {
                process_disconnect(
                    host_anchor,
                    &mut sessions,
                    &dim_channels,
                    &mut mcrs_minecraft_level::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );

                // Second call routes nothing (the session is already unplaced).
                process_disconnect(
                    host_anchor,
                    &mut sessions,
                    &dim_channels,
                    &mut mcrs_minecraft_level::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );
            },
        )
        .expect("system runs without panicking");

    app.update();

    assert_eq!(session_count(&mut app), 0, "no session left after cleanup");

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "host-anchor entity despawned after cleanup",
    );

    assert!(
        sessions_named(&mut app, "test_player").is_empty(),
        "the username names no session after cleanup",
    );

    let despawn_msgs: Vec<_> = ctl_rx.try_iter().collect();
    assert_eq!(
        despawn_msgs.len(),
        1,
        "exactly one despawn routed to control channel (second call routes nothing)",
    );
    match &despawn_msgs[0] {
        mcrs_minecraft_server::world::channel_types::ToDim::Despawn {
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
fn a_late_cleanup_leaves_the_name_to_the_player_who_logged_in_again() {
    let mut app = make_app();
    let first = insert_accepted_login(&mut app, fresh_profile());
    let second = insert_accepted_login(&mut app, fresh_profile());
    let first_anchor = anchor_of(&app, first);
    let second_session = app
        .world()
        .get::<Session>(anchor_of(&app, second))
        .expect("session present")
        .0;

    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut sessions: LeavingSessions,
                  dim_channels: Res<DimChannelsResource>| {
                process_disconnect(
                    first_anchor,
                    &mut sessions,
                    &dim_channels,
                    &mut mcrs_minecraft_level::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );
            },
        )
        .expect("system runs without panicking");

    assert_eq!(
        sessions_named(&mut app, "test_player"),
        vec![second_session]
    );
}

#[test]
fn login_observer_no_ops_for_non_accepted_login_states() {
    let mut app = make_app();
    let entity = app.world_mut().spawn_empty().id();

    app.world_mut()
        .entity_mut(entity)
        .insert((fresh_profile(), LoginState::Hello));
    app.update();

    assert_eq!(
        session_count(&mut app),
        0,
        "no session for non-Accepted login state",
    );
    assert!(
        app.world().entity(entity).get::<HostAnchorRef>().is_none(),
        "no HostAnchorRef attached for non-Accepted login state",
    );
}
