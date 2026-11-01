use bevy_app::{App, FixedPreUpdate};
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{
    BoundAddress, NetworkPlugin, PROTOCOL_26_3, PROTOCOL_26_4_SNAPSHOT_5, ServerSideConnection,
};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use mcrs_minecraft_text::Text;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

fn server(accepts_transfers: bool) -> (App, SocketAddr) {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
        accepts_transfers,
        ..Default::default()
    });
    let address = app.world().resource::<BoundAddress>().0;
    app.update();
    (app, address)
}

async fn handshake(
    address: SocketAddr,
    protocol_version: i32,
    intent: Intent,
) -> PacketIo<tokio::net::TcpStream> {
    let mut io = PacketIo::connect(address).await.unwrap();
    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(protocol_version),
        server_address: Bounded("127.0.0.1"),
        server_port: address.port(),
        intent,
    })
    .await
    .unwrap();
    io
}

fn refusal(address: SocketAddr, protocol_version: i32, intent: Intent) -> Text {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut io = handshake(address, protocol_version, intent).await;
            let packet = io
                .recv_packet::<ClientboundLoginDisconnect>()
                .await
                .unwrap();
            serde_json::from_str::<Text>(packet.reason.0).unwrap()
        })
        .await
        .unwrap()
    })
}

#[test]
fn a_login_under_another_protocol_version_is_refused_with_the_reason_of_its_age() {
    let (_app, address) = server(true);
    for intent in [Intent::Login, Intent::Transfer] {
        for (protocol_version, key) in [
            (753, "multiplayer.disconnect.outdated_client"),
            (778, "multiplayer.disconnect.incompatible"),
        ] {
            assert_eq!(
                refusal(address, protocol_version, intent),
                Text::translate(key, vec![Text::from(VERSION.name)]),
                "{intent:?} {protocol_version}"
            );
        }
    }
}

#[test]
fn a_login_under_26_3_the_own_version_or_26_4_snapshot_5_reaches_the_server() {
    let (mut app, address) = server(true);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let handshakes: Vec<_> = [Intent::Login, Intent::Transfer]
        .into_iter()
        .flat_map(|intent| {
            [
                PROTOCOL_26_3,
                VERSION.protocol_version,
                PROTOCOL_26_4_SNAPSHOT_5,
            ]
            .map(|protocol_version| (protocol_version, intent))
        })
        .collect();
    let _peers: Vec<_> = handshakes
        .iter()
        .map(|&(protocol_version, intent)| {
            runtime.block_on(handshake(address, protocol_version, intent))
        })
        .collect();

    let mut connections = app.world_mut().query::<&ServerSideConnection>();
    for _ in 0..200 {
        app.world_mut().run_schedule(FixedPreUpdate);
        if connections.iter(app.world()).count() == handshakes.len() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("not every accepted version reached the server");
}

#[test]
fn a_transfer_is_refused_before_its_version_is_checked_when_transfers_are_off() {
    let (_app, address) = server(false);
    for protocol_version in [VERSION.protocol_version, 778] {
        assert_eq!(
            refusal(address, protocol_version, Intent::Transfer),
            Text::translate("multiplayer.disconnect.transfers_disabled", vec![]),
            "{protocol_version}"
        );
    }
}
