use bevy_app::{App, FixedPreUpdate};
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{
    BoundAddress, NetworkPlugin, PROTOCOL_26_3, PROTOCOL_26_4_SNAPSHOT_5, ServerSideConnection,
};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginDisconnect;
use mcrs_minecraft_protocol::{Bounded, Text, VarInt};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

#[test]
fn a_login_under_another_protocol_version_is_refused_with_the_reason_of_its_age() {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
    });
    let address = app.world().resource::<BoundAddress>().0;
    app.update();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    for (protocol_version, key) in [
        (753, "multiplayer.disconnect.outdated_client"),
        (778, "multiplayer.disconnect.incompatible"),
    ] {
        let reason = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let mut io = PacketIo::connect(address).await.unwrap();
                io.send_packet(&ServerboundHandshake {
                    protocol_version: VarInt(protocol_version),
                    server_address: Bounded("127.0.0.1"),
                    server_port: address.port(),
                    intent: Intent::Login,
                })
                .await
                .unwrap();
                let packet = io
                    .recv_packet::<ClientboundLoginDisconnect>()
                    .await
                    .unwrap();
                serde_json::from_str::<Text>(packet.reason.0).unwrap()
            })
            .await
            .unwrap()
        });
        assert_eq!(
            reason,
            Text::translate(key, vec![Text::from(VERSION.name.clone())]),
            "{protocol_version}"
        );
    }
}

#[test]
fn a_login_under_26_3_the_own_version_or_26_4_snapshot_5_reaches_the_server() {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
    });
    let address = app.world().resource::<BoundAddress>().0;
    app.update();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let versions = [
        PROTOCOL_26_3,
        VERSION.protocol_version,
        PROTOCOL_26_4_SNAPSHOT_5,
    ];
    let _peers: Vec<_> = versions
        .iter()
        .map(|&protocol_version| {
            runtime.block_on(async {
                let mut io = PacketIo::connect(address).await.unwrap();
                io.send_packet(&ServerboundHandshake {
                    protocol_version: VarInt(protocol_version),
                    server_address: Bounded("127.0.0.1"),
                    server_port: address.port(),
                    intent: Intent::Login,
                })
                .await
                .unwrap();
                io
            })
        })
        .collect();

    let mut connections = app.world_mut().query::<&ServerSideConnection>();
    for _ in 0..200 {
        app.world_mut().run_schedule(FixedPreUpdate);
        if connections.iter(app.world()).count() == versions.len() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("not every accepted version reached the server");
}
