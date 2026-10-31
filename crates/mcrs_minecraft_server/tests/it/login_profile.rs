use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{IntoSystem, System};
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft_engine::dim::pump_channels;
use mcrs_minecraft_engine::disconnect::DisconnectProtocolPlugin;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_engine::session::HostAnchorRef;
use mcrs_minecraft_engine_server::configuration::on_configuration_ack;
use mcrs_minecraft_engine_server::login::{LoginPlugin, LoginState, SingleplayerProfile};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_minecraft_network::event::run_event_loop;
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_network::{ConnectionState, ReceivedPacket, ServerSideConnection, SwitchMode};
use mcrs_minecraft_protocol::decode::{PacketDecoder, PacketFrame};
use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundFinishConfiguration,
};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished, ClientboundLoginFinishedWithOptions,
    LoginCompression, OptionFlags,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::uuid::{Uuid, uuid};
use mcrs_minecraft_protocol::{
    Bounded, CompressionThreshold, Decode, Encode, Packet, Text, VarInt, WritePacket,
};
use tokio::sync::mpsc;

use crate::mock_connection;

const OPERATOR: Uuid = Uuid::from_u128(0x0bad_cafe_0000_4000_8000_0000_0000_0001);

/// The game's offline ids of `Steve` and `Guest`.
const STEVE: Uuid = uuid!("5627dd98-e6be-3c21-b8a8-e92344183641");
const GUEST: Uuid = uuid!("404198aa-cc64-3021-9fdf-8766c0d0096c");

fn server(host: Option<SingleplayerProfile>) -> App {
    let mut app = App::new();
    app.add_plugins(LoginPlugin);
    app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
    app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
    app.add_message::<InboundPlayerDespawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.init_resource::<DimChannelsResource>();
    app.add_plugins(DisconnectProtocolPlugin);
    if let Some(host) = host {
        app.insert_resource(host);
    }
    app
}

struct Client {
    connection: Entity,
    outgoing: mpsc::Receiver<Bytes>,
    decoder: PacketDecoder,
    switch_mode: SwitchMode,
}

impl Client {
    fn received(&mut self, app: &mut App) -> Vec<PacketFrame> {
        if let Some(mut con) = app
            .world_mut()
            .get_mut::<ServerSideConnection>(self.connection)
        {
            con.raw.flush().unwrap();
        }
        while let Ok(blob) = self.outgoing.try_recv() {
            self.decoder.queue_bytes(BytesMut::from(&blob[..]));
        }
        std::iter::from_fn(|| {
            let frame = self.decoder.try_next_packet().unwrap()?;
            if frame.id == LoginCompression::ID && self.switch_mode == SwitchMode::Immediate {
                let packet = LoginCompression::decode(&mut &frame.body[..]).unwrap();
                self.decoder
                    .set_compression(CompressionThreshold(packet.threshold.0));
            }
            Some(frame)
        })
        .collect()
    }
}

fn connect(app: &mut App) -> Client {
    connect_with(app, SwitchMode::Immediate)
}

fn connect_with(app: &mut App, switch_mode: SwitchMode) -> Client {
    let (raw, outgoing, _inbound) = mock_connection::make_mock_raw_connection_full();
    let raw = raw.with_switch_mode(switch_mode);
    let connection = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Login,
        ))
        .id();
    Client {
        connection,
        outgoing,
        decoder: PacketDecoder::new(),
        switch_mode,
    }
}

fn hello(app: &mut App, username: &str, profile_id: Uuid) -> GameProfile {
    let client = connect(app);
    send_hello(app, &client, username, profile_id);
    app.world()
        .get::<GameProfile>(client.connection)
        .expect("the hello is accepted")
        .clone()
}

fn send_hello(app: &mut App, client: &Client, username: &str, profile_id: Uuid) {
    send(
        app,
        client,
        &ServerboundHello {
            username: Bounded(username),
            profile_id,
        },
    );
}

fn send<P: Encode + Packet>(app: &mut App, client: &Client, packet: &P) {
    let mut data = Vec::new();
    packet.encode(&mut data).unwrap();
    let trigger = PacketDispatcher::serverbound()
        .dispatch(P::STATE, P::ID, &data)
        .unwrap()
        .unwrap();
    trigger.raise(
        &mut app.world_mut().commands(),
        client.connection,
        data.into(),
        mcrs_minecraft_network::Instant::now(),
    );
    app.world_mut().flush();
}

/// A dimension whose control messages the test reads and whose replies it writes.
fn dimension(app: &mut App) -> (Entity, flume::Receiver<ToDim>, flume::Sender<FromDim>) {
    let dim = app.world_mut().spawn_empty().id();
    let (serverbound, _) = flume::bounded(TO_DIM_CAPACITY);
    let (control, to_dim) = flume::bounded(TO_DIM_CONTROL_CAPACITY);
    let (from_dim, received) = flume::bounded(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(dim, serverbound, control, received);
    (dim, to_dim, from_dim)
}

fn anchor(app: &App, client: &Client) -> Entity {
    app.world()
        .get::<HostAnchorRef>(client.connection)
        .expect("the login is accepted")
        .0
}

fn sessions_under(app: &mut App, id: Uuid) -> Vec<Entity> {
    app.world_mut()
        .query::<(Entity, &GameProfile, &Session)>()
        .iter(app.world())
        .filter(|(_, profile, _)| profile.id == id)
        .map(|(anchor, ..)| anchor)
        .collect()
}

fn connected(app: &App, client: &Client) -> bool {
    app.world()
        .get::<ServerSideConnection>(client.connection)
        .is_some()
}

fn ids(frames: &[PacketFrame]) -> Vec<i32> {
    frames.iter().map(|frame| frame.id).collect()
}

fn duplicate_login() -> Text {
    Text::translate("multiplayer.disconnect.duplicate_login", Vec::new())
}

fn play_disconnect_reason(frames: &[PacketFrame]) -> Text {
    let [frame] = frames else {
        panic!("{frames:?}");
    };
    assert_eq!(frame.id, ClientboundDisconnect::ID);
    ClientboundDisconnect::decode(&mut &frame.body[..])
        .unwrap()
        .reason
}

fn login_disconnect_reason(frames: &[PacketFrame]) -> serde_json::Value {
    let [frame] = frames else {
        panic!("{frames:?}");
    };
    assert_eq!(frame.id, ClientboundLoginDisconnect::ID);
    let packet = ClientboundLoginDisconnect::decode(&mut &frame.body[..]).unwrap();
    serde_json::from_str(packet.reason.0).unwrap()
}

#[test]
fn a_hello_plays_under_the_offline_id_of_its_name_whatever_id_it_carries() {
    let mut app = server(None);
    let claimed = hello(&mut app, "Steve", OPERATOR);
    let other = hello(&mut app, "Steve", Uuid::new_v4());
    assert_eq!(claimed.id, STEVE);
    assert_eq!(other.id, claimed.id);
    assert_ne!(claimed.id, OPERATOR);
    assert_eq!(claimed.username, "Steve");
}

#[test]
fn an_acknowledged_switch_login_compresses_only_after_the_acknowledgement() {
    let mut app = server(None);
    let mut client = connect_with(&mut app, SwitchMode::Acknowledged);
    send_hello(&mut app, &client, "Steve", Uuid::new_v4());
    let frames = client.received(&mut app);
    assert_eq!(
        ids(&frames),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );
    let finished = ClientboundLoginFinishedWithOptions::decode(&mut &frames[1].body[..]).unwrap();
    assert_eq!(finished.options, OptionFlags::new().with_compression(true));

    send(&mut app, &client, &ServerboundLoginAcknowledged);
    let reason = "a".repeat(1000);
    let large = ClientboundLoginDisconnect {
        reason: Bounded(&reason),
    };
    let mut con = app
        .world_mut()
        .get_mut::<ServerSideConnection>(client.connection)
        .unwrap();
    con.write_packet(&large);
    con.raw.flush().unwrap();
    let mut wire = BytesMut::from(&client.outgoing.try_recv().unwrap()[..]);
    let frame = mcrs_minecraft_protocol::frame::split_frame(&mut wire).unwrap();
    let mut uncompressed = Vec::new();
    large.encode_with_id(&mut uncompressed).unwrap();
    assert_eq!(
        VarInt::decode(&mut &frame[..]).unwrap().0 as usize,
        uncompressed.len()
    );
}

#[test]
fn an_acknowledged_switch_login_that_times_out_after_login_finished_closes_without_a_packet() {
    let mut app = server(None);
    let mut client = connect_with(&mut app, SwitchMode::Acknowledged);
    send_hello(&mut app, &client, "Steve", Uuid::new_v4());
    client.received(&mut app);

    for _ in 0..=600 {
        app.update();
    }
    assert!(!connected(&app, &client));
    assert!(client.received(&mut app).is_empty());
}

#[test]
fn a_hello_under_the_hosts_name_in_any_case_plays_as_the_host() {
    let host = SingleplayerProfile {
        name: "Host".to_owned(),
        id: OPERATOR,
    };
    let mut app = server(Some(host.clone()));

    let owner = hello(&mut app, "hOST", Uuid::new_v4());
    assert_eq!((owner.id, owner.username.as_str()), (host.id, "Host"));

    let guest = hello(&mut app, "Guest", host.id);
    assert_eq!(guest.id, GUEST);
}

#[test]
fn a_hello_under_a_name_the_game_refuses_is_disconnected_with_its_reason() {
    let mut app = server(None);
    for name in ["St eve", "Stеve", "Steve\n", "§cSteve"] {
        let mut client = connect(&mut app);
        send_hello(&mut app, &client, name, Uuid::new_v4());

        let world = app.world();
        assert!(
            world.get::<GameProfile>(client.connection).is_none(),
            "{name:?}"
        );
        assert!(
            world
                .get::<ServerSideConnection>(client.connection)
                .is_none(),
            "{name:?} is still connected"
        );
        let received = client.received(&mut app);
        let [frame] = received.as_slice() else {
            panic!("{name:?}: {received:?}");
        };
        assert_eq!(frame.id, ClientboundLoginDisconnect::ID, "{name:?}");
        let packet = ClientboundLoginDisconnect::decode(&mut &frame.body[..]).unwrap();
        let reason: serde_json::Value = serde_json::from_str(packet.reason.0).unwrap();
        assert_eq!(
            reason,
            serde_json::json!({
                "translate": "disconnect.genericReason",
                "with": [
                    "Internal Exception: java.lang.IllegalStateException: Invalid characters in username"
                ],
            }),
            "{name:?}"
        );
    }
}

#[test]
fn a_hello_under_an_empty_name_is_accepted_as_in_the_game() {
    let mut app = server(None);
    assert_eq!(hello(&mut app, "", Uuid::new_v4()).username, "");
}

#[test]
fn a_second_login_under_a_name_ends_the_first_session_and_plays_once_its_player_is_saved() {
    let mut app = server(None);
    let (dim, to_dim, from_dim) = dimension(&mut app);

    let mut first = connect(&mut app);
    send_hello(&mut app, &first, "Steve", Uuid::new_v4());
    assert_eq!(
        ids(&first.received(&mut app)),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );
    let first_anchor = anchor(&app, &first);
    app.world_mut()
        .entity_mut(first.connection)
        .insert(ConnectionState::Game);
    app.world_mut()
        .entity_mut(first_anchor)
        .insert(SessionPlacement::new(Place::InDim(dim), 0));

    let mut second = connect(&mut app);
    send_hello(&mut app, &second, "Steve", Uuid::new_v4());

    assert_eq!(
        play_disconnect_reason(&first.received(&mut app)),
        duplicate_login()
    );
    assert!(!connected(&app, &first));
    let Ok(ToDim::Despawn(despawn)) = to_dim.try_recv() else {
        panic!("the first player's dimension is not told to let it go");
    };
    assert_eq!(despawn.host_anchor, first_anchor);

    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        ids(&second.received(&mut app)),
        [LoginCompression::ID],
        "the second login finishes before the first player is saved"
    );
    assert_eq!(
        app.world().get::<LoginState>(second.connection),
        Some(&LoginState::AwaitingDeparture)
    );
    assert!(sessions_under(&mut app, STEVE).is_empty());

    from_dim
        .send(FromDim::Released {
            session: despawn.session,
        })
        .unwrap();
    pump_channels(&mut app);
    app.update();

    assert_eq!(
        ids(&second.received(&mut app)),
        [ClientboundLoginFinished::ID]
    );
    assert!(connected(&app, &second));
    assert_eq!(sessions_under(&mut app, STEVE), [anchor(&app, &second)]);
}

#[test]
fn a_connection_silent_in_login_is_disconnected_after_six_hundred_ticks() {
    let mut app = server(None);
    let mut client = connect(&mut app);

    for _ in 0..600 {
        app.update();
    }
    assert!(connected(&app, &client));

    app.update();
    assert!(!connected(&app, &client));
    assert_eq!(
        login_disconnect_reason(&client.received(&mut app)),
        serde_json::json!({"translate": "multiplayer.disconnect.slow_login"})
    );
}

#[test]
fn a_login_held_for_a_save_that_never_comes_ends_after_six_hundred_ticks() {
    let mut app = server(None);
    let (dim, to_dim, _from_dim) = dimension(&mut app);

    let mut first = connect(&mut app);
    send_hello(&mut app, &first, "Steve", Uuid::new_v4());
    first.received(&mut app);
    let first_anchor = anchor(&app, &first);
    app.world_mut()
        .entity_mut(first.connection)
        .insert(ConnectionState::Game);
    app.world_mut()
        .entity_mut(first_anchor)
        .insert(SessionPlacement::new(Place::InDim(dim), 0));

    let mut second = connect(&mut app);
    send_hello(&mut app, &second, "Steve", Uuid::new_v4());
    assert!(matches!(to_dim.try_recv(), Ok(ToDim::Despawn(_))));

    for _ in 0..600 {
        app.update();
    }
    assert!(connected(&app, &second));
    assert_eq!(ids(&second.received(&mut app)), [LoginCompression::ID]);

    app.update();
    assert!(!connected(&app, &second));
    assert_eq!(
        login_disconnect_reason(&second.received(&mut app)),
        serde_json::json!({"translate": "multiplayer.disconnect.slow_login"})
    );

    let mut third = connect(&mut app);
    send_hello(&mut app, &third, "Steve", Uuid::new_v4());
    assert_eq!(
        ids(&third.received(&mut app)),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );
    assert_eq!(sessions_under(&mut app, STEVE), [anchor(&app, &third)]);
}

#[test]
fn a_login_under_the_hosts_name_in_another_case_ends_the_hosts_session() {
    let host = SingleplayerProfile {
        name: "Host".to_owned(),
        id: OPERATOR,
    };
    let mut app = server(Some(host));

    let mut owner = connect(&mut app);
    send_hello(&mut app, &owner, "Host", Uuid::new_v4());
    assert_eq!(
        ids(&owner.received(&mut app)),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );

    app.world_mut()
        .entity_mut(owner.connection)
        .insert(ConnectionState::Game);

    let mut guest = connect(&mut app);
    send_hello(&mut app, &guest, "hOST", Uuid::new_v4());

    assert_eq!(
        play_disconnect_reason(&owner.received(&mut app)),
        duplicate_login()
    );
    assert!(!connected(&app, &owner));

    app.update();
    assert_eq!(
        ids(&guest.received(&mut app)),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );
    assert_eq!(sessions_under(&mut app, OPERATOR), [anchor(&app, &guest)]);
}

#[test]
fn a_second_login_under_a_name_leaves_a_session_that_is_not_yet_in_the_world() {
    let mut app = server(None);

    let mut first = connect(&mut app);
    send_hello(&mut app, &first, "Steve", Uuid::new_v4());
    first.received(&mut app);

    let mut second = connect(&mut app);
    send_hello(&mut app, &second, "Steve", Uuid::new_v4());

    assert!(first.received(&mut app).is_empty());
    assert!(connected(&app, &first));
    assert_eq!(
        ids(&second.received(&mut app)),
        [LoginCompression::ID, ClientboundLoginFinished::ID]
    );
}

#[test]
fn a_session_finishing_configuration_while_another_under_its_id_plays_is_refused() {
    let mut app = server(None);
    app.add_observer(on_configuration_ack);
    let profile = GameProfile {
        id: STEVE,
        username: "Steve".to_owned(),
        properties: Vec::new(),
    };
    let mut clients: Vec<Client> = (0..2).map(|_| connect(&mut app)).collect();
    for client in &clients {
        app.world_mut().entity_mut(client.connection).insert((
            profile.clone(),
            LoginState::Accepted,
            ConnectionState::Configuration,
        ));
    }
    app.world_mut().flush();

    send(&mut app, &clients[0], &ServerboundFinishConfiguration);
    send(&mut app, &clients[1], &ServerboundFinishConfiguration);

    assert_eq!(
        app.world().get::<ConnectionState>(clients[0].connection),
        Some(&ConnectionState::Game)
    );
    assert!(clients[0].received(&mut app).is_empty());
    assert_eq!(
        app.world().get::<ConnectionState>(clients[1].connection),
        Some(&ConnectionState::Configuration)
    );
    assert_eq!(
        play_disconnect_reason(&clients[1].received(&mut app)),
        duplicate_login()
    );
    assert!(!connected(&app, &clients[1]));
}

fn frame<P: Encode + Packet>(packet: &P) -> ReceivedPacket {
    let mut body = Vec::new();
    packet.encode(&mut body).unwrap();
    ReceivedPacket {
        timestamp: mcrs_minecraft_network::Instant::now(),
        id: P::ID,
        payload: body.into(),
    }
}

fn read_sockets(app: &mut App) {
    let mut system = IntoSystem::into_system(run_event_loop);
    system.initialize(app.world_mut());
    let _ = system.run((), app.world_mut());
    system.apply_deferred(app.world_mut());
}

#[test]
fn configuration_frames_sent_with_the_login_acknowledgement_count_no_decode_fault() {
    let mut app = server(None);
    app.insert_resource(PacketDispatcher::serverbound());
    let (raw, _outgoing, inbound) = mock_connection::make_mock_raw_connection_full();
    let connection = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Login,
        ))
        .id();

    let hello = ServerboundHello {
        username: Bounded("Steve"),
        profile_id: Uuid::new_v4(),
    };
    inbound.try_send(frame(&hello)).unwrap();
    read_sockets(&mut app);
    assert_eq!(
        app.world().get::<LoginState>(connection),
        Some(&LoginState::Accepted)
    );

    inbound
        .try_send(frame(&ServerboundLoginAcknowledged))
        .unwrap();
    inbound
        .try_send(frame(&ServerboundClientInformation(ClientInformation {
            locale: "en_us",
            view_distance: 12,
            chat_mode: ChatMode::Enabled,
            chat_colors: true,
            displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
            main_arm: MainArm::Right,
            enable_text_filtering: false,
            allow_server_listings: true,
            particle_status: ParticleStatus::All,
        })))
        .unwrap();
    read_sockets(&mut app);
    read_sockets(&mut app);

    assert_eq!(
        app.world().get::<ConnectionState>(connection),
        Some(&ConnectionState::Configuration)
    );
    let mut socket = app
        .world_mut()
        .get_mut::<ServerSideConnection>(connection)
        .unwrap();
    assert!(matches!(socket.raw.try_recv(), Ok(None)));
    let counts = app.world().resource::<PacketDispatcher>();
    for state in [ConnectionState::Login, ConnectionState::Configuration] {
        let counts = counts.counts(state).unwrap();
        assert_eq!(
            (counts.failures_total(), counts.unknown()),
            (0, 0),
            "{state:?}"
        );
    }
}
