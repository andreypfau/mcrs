use bevy_app::App;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::packets::login::serverbound::ServerboundHello;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Encode, Packet};
use mcrs_minecraft_server::login::{GameProfile, LoginPlugin, SingleplayerProfile};
use mcrs_minecraft_server::world::bus::InboundPlayerDespawn;
use md5::{Digest, Md5};

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

fn hello(app: &mut App, username: &str, profile_id: Uuid) -> GameProfile {
    let (raw, _outgoing, _inbound) = mock_connection::make_mock_raw_connection_full();
    let connection: Entity = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Login,
        ))
        .id();
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
    app.world()
        .get::<GameProfile>(connection)
        .expect("the hello is accepted")
        .clone()
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
