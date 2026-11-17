use crate::client_join::StopOnDrop;
use crate::configuration_registries::ASSETS;
use bevy_app::{App, AppExit, PluginGroup, Update};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::Changed;
use bevy_ecs::system::Query;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_server::{BoundAddress, ForwardingSecret, run_server_loop};
use mcrs_minecraft_network::join::{Configuration, client_information};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{ConnectionState, FORWARDING_CHANNEL, ServerSideConnection};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::configuration::serverbound::{
    ServerboundClientInformation, ServerboundFinishConfiguration,
};
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginFinishedWithOptions, CustomQuery,
};
use mcrs_minecraft_protocol::packets::login::serverbound::{
    ServerboundCustomQueryAnswer, ServerboundHello, ServerboundLoginAcknowledged,
};
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::uuid::{Uuid, uuid};
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, RawBytes, VarInt};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_vanilla_light::LightingPlugin;
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) const SECRET: &str = "example-secret";
pub(crate) const FORWARDED_ID: Uuid = uuid!("00112233-4455-6677-8899-aabbccddeeff");
pub(crate) const FORWARDED_ADDRESS: &str = "203.0.113.7";

pub(crate) const FORWARDED_PAYLOAD: [u8; 83] = [
    0x0b, 0xe1, 0x3f, 0x34, 0x75, 0x9c, 0x06, 0x93, 0x4e, 0x5a, 0x85, 0x85, 0x15, 0x68, 0x73, 0x1b,
    0xf3, 0xaa, 0x52, 0x67, 0xc3, 0xf7, 0x51, 0x1e, 0x0b, 0x33, 0xd1, 0x6e, 0xa4, 0xb8, 0x14, 0x59,
    0x04, 0x0b, 0x32, 0x30, 0x33, 0x2e, 0x30, 0x2e, 0x31, 0x31, 0x33, 0x2e, 0x37, 0x00, 0x11, 0x22,
    0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x05, 0x53, 0x74,
    0x65, 0x76, 0x65, 0x01, 0x08, 0x74, 0x65, 0x78, 0x74, 0x75, 0x72, 0x65, 0x73, 0x04, 0x65, 0x33,
    0x30, 0x3d, 0x00,
];

pub(crate) fn forwarded_properties() -> Vec<Property> {
    vec![Property {
        name: "textures".to_owned(),
        value: "e30=".to_owned(),
        signature: None,
    }]
}

#[test]
fn fwd_use_003_a_proxy_logs_a_player_in_under_the_forwarded_profile_and_address() {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network.announce_on_lan = false;
    plugins.network.forwarding = Some(ForwardingSecret::new(SECRET).unwrap());
    let mut app = App::new();
    app.add_plugins(
        plugins
            .build()
            .disable::<WorldGenPlugin>()
            .disable::<LightingPlugin>(),
    );
    let (joined_send, joined_recv) = std::sync::mpsc::channel();
    app.add_systems(
        Update,
        move |players: Query<
            (&ConnectionState, &GameProfile, &ServerSideConnection),
            Changed<ConnectionState>,
        >| {
            for (state, profile, connection) in &players {
                if *state == ConnectionState::Game {
                    joined_send
                        .send((profile.clone(), connection.remote_addr()))
                        .ok();
                }
            }
        },
    );
    let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
    let stopped = stop.0.clone();
    app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stopped.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let address = app.world().resource::<BoundAddress>().0;
    let server = mcrs_minecraft_engine_server::spawn_server_thread(app, |app| {
        run_server_loop(app);
    })
    .expect("the server thread starts");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (login_finished, proxy) = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), fakeproxy(address))
            .await
            .expect("the forwarded login within the timeout")
    });
    let (profile, remote_addr) = joined_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the forwarded player reaches the game state");
    drop(stop);
    server.join().unwrap();
    drop(proxy);

    assert_eq!(
        (login_finished.0, login_finished.1.as_str()),
        (FORWARDED_ID, "Steve")
    );
    assert_eq!(login_finished.2, forwarded_properties());
    assert_eq!(
        (profile.id, profile.username.as_str()),
        (FORWARDED_ID, "Steve")
    );
    assert_eq!(profile.properties, forwarded_properties());
    assert_eq!(remote_addr.ip().to_string(), FORWARDED_ADDRESS);
}

async fn fakeproxy(
    address: SocketAddr,
) -> (
    (Uuid, String, Vec<Property>),
    PacketIo<tokio::net::TcpStream>,
) {
    let mut io = PacketIo::connect(address).await.unwrap();
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(VERSION.protocol_version),
        server_address: Bounded("127.0.0.1"),
        server_port: address.port(),
        intent: Intent::Login,
    })
    .await
    .unwrap();
    io.send_packet(&ServerboundHello {
        username: Bounded("ProxyName"),
        profile_id: Uuid::nil(),
    })
    .await
    .unwrap();

    let (id, body) = io.recv_frame().await.unwrap();
    assert_eq!(id, CustomQuery::ID);
    let query = CustomQuery::decode(&mut &body[..]).unwrap();
    assert_eq!(query.channel, FORWARDING_CHANNEL);
    assert_eq!(query.data.0.0, [4]);
    io.send_packet(&ServerboundCustomQueryAnswer {
        message_id: query.message_id,
        payload: Some(Bounded(RawBytes(&FORWARDED_PAYLOAD))),
    })
    .await
    .unwrap();

    let (id, body) = io.recv_frame().await.unwrap();
    assert_eq!(id, ClientboundLoginFinishedWithOptions::ID);
    let finished = ClientboundLoginFinishedWithOptions::decode(&mut &body[..]).unwrap();
    let profile = finished.profile;
    let profile = (
        profile.id,
        profile.username.0.to_owned(),
        profile
            .properties
            .iter()
            .map(|property| Property {
                name: property.name.to_owned(),
                value: property.value.to_owned(),
                signature: property.signature.map(str::to_owned),
            })
            .collect(),
    );
    io.send_packet(&ServerboundLoginAcknowledged).await.unwrap();
    io.send_packet(&ServerboundClientInformation(client_information(2)))
        .await
        .unwrap();
    let mut configuration = Configuration::default();
    loop {
        let (id, body) = io.recv_frame().await.unwrap();
        let finished = configuration.read(id, &body, false, &mut io).unwrap();
        io.flush().await.unwrap();
        if finished.is_some() {
            break;
        }
    }
    io.send_packet(&ServerboundFinishConfiguration)
        .await
        .unwrap();
    (profile, io)
}
