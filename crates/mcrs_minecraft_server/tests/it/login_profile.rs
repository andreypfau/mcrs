use bevy_app::App;
use bevy_ecs::entity::Entity;
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};
use mcrs_minecraft_level::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::decode::{PacketDecoder, PacketFrame};
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginDisconnect, ClientboundLoginFinished,
};
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundHello;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, Packet, Text};
use mcrs_minecraft_server::configuration::on_configuration_ack;
use mcrs_minecraft_server::dim::pump_channels;
use mcrs_minecraft_server::disconnect::DisconnectProtocolPlugin;
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, LoginState, SingleplayerProfile};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft_server::world::session::HostAnchorRef;
use md5::{Digest, Md5};
use tokio::sync::mpsc;

use crate::mock_connection;

const OPERATOR: Uuid = Uuid::from_u128(0x0bad_cafe_0000_4000_8000_0000_0000_0001);

/// `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`: an MD5 of the bytes with
/// the version set to 3 and the variant to IETF.
fn reference_offline_id(name: &str) -> Uuid {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

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
        std::iter::from_fn(|| self.decoder.try_next_packet().unwrap()).collect()
    }
}

fn connect(app: &mut App) -> Client {
    let (raw, outgoing, _inbound) = mock_connection::make_mock_raw_connection_full();
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
    app.world_mut().trigger(ReceivedPacketEvent {
        entity: client.connection,
        id: P::ID,
        data: data.into(),
        timestamp: mcrs_minecraft_network::Instant::now(),
    });
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

#[test]
fn a_hello_plays_under_the_offline_id_of_its_name_whatever_id_it_carries() {
    let mut app = server(None);
    let claimed = hello(&mut app, "Steve", OPERATOR);
    let other = hello(&mut app, "Steve", Uuid::new_v4());
    assert_eq!(claimed.id, reference_offline_id("Steve"));
    assert_eq!(other.id, claimed.id);
    assert_ne!(claimed.id, OPERATOR);
    assert_eq!(claimed.username, "Steve");
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
    assert_eq!(guest.id, reference_offline_id("Guest"));
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
    let steve = reference_offline_id("Steve");

    let mut first = connect(&mut app);
    send_hello(&mut app, &first, "Steve", Uuid::new_v4());
    assert_eq!(
        ids(&first.received(&mut app)),
        [ClientboundLoginFinished::ID]
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
    assert!(
        second.received(&mut app).is_empty(),
        "the second login finishes before the first player is saved"
    );
    assert_eq!(
        app.world().get::<LoginState>(second.connection),
        Some(&LoginState::AwaitingDeparture)
    );
    assert!(sessions_under(&mut app, steve).is_empty());

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
    assert_eq!(sessions_under(&mut app, steve), [anchor(&app, &second)]);
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
        [ClientboundLoginFinished::ID]
    );

    let mut guest = connect(&mut app);
    send_hello(&mut app, &guest, "hOST", Uuid::new_v4());

    let received = owner.received(&mut app);
    let [frame] = received.as_slice() else {
        panic!("{received:?}");
    };
    assert_eq!(frame.id, ClientboundLoginDisconnect::ID);
    let packet = ClientboundLoginDisconnect::decode(&mut &frame.body[..]).unwrap();
    let reason: serde_json::Value = serde_json::from_str(packet.reason.0).unwrap();
    assert_eq!(
        reason,
        serde_json::json!({"translate": "multiplayer.disconnect.duplicate_login"})
    );
    assert!(!connected(&app, &owner));

    app.update();
    assert_eq!(
        ids(&guest.received(&mut app)),
        [ClientboundLoginFinished::ID]
    );
    assert_eq!(sessions_under(&mut app, OPERATOR), [anchor(&app, &guest)]);
}

#[test]
fn a_session_finishing_configuration_while_another_under_its_id_plays_is_refused() {
    let mut app = server(None);
    app.add_observer(on_configuration_ack);
    let profile = GameProfile {
        id: reference_offline_id("Steve"),
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
