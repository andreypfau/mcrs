use bevy_app::App;
use bevy_ecs::entity::Entity;
use bytes::{Bytes, BytesMut};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::decode::{PacketDecoder, PacketFrame};
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundHello;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Encode, Packet};
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, SingleplayerProfile};
use mcrs_minecraft_server::world::bus::InboundPlayerDespawn;
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
    fn received(&mut self) -> Vec<PacketFrame> {
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
    let connection = client.connection;
    let mut data = Vec::new();
    ServerboundHello {
        username: Bounded(username),
        profile_id,
    }
    .encode(&mut data)
    .unwrap();
    app.world_mut().trigger(ReceivedPacketEvent {
        entity: connection,
        id: ServerboundHello::ID,
        data: data.into(),
        timestamp: mcrs_minecraft_network::Instant::now(),
    });
    app.world_mut().flush();
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
        let received = client.received();
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
