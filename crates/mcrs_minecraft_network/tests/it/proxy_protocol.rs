use bevy_app::App;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{BoundAddress, NetworkPlugin};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

const V2_LOCAL: &[u8] = b"\r\n\r\n\0\r\nQUIT\n\x20\x00\x00\x00";

fn server(upstream: IpAddr) -> (App, SocketAddr) {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
        proxy_upstreams: vec![upstream],
        ..Default::default()
    });
    let address = app.world().resource::<BoundAddress>().0;
    app.update();
    (app, address)
}

fn v1(source: &str) -> Vec<u8> {
    format!("PROXY TCP4 {source} 192.0.2.10 51234 25565\r\n").into_bytes()
}

fn answered(address: SocketAddr, header: &[u8]) -> bool {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let exchange = async {
            let mut stream = TcpStream::connect(address).await?;
            stream.write_all(header).await?;
            let mut io = PacketIo::new(stream);
            io.send_packet(&ServerboundHandshake {
                protocol_version: VarInt(VERSION.protocol_version),
                server_address: Bounded("127.0.0.1"),
                server_port: address.port(),
                intent: Intent::Status,
            })
            .await?;
            io.send_packet(&StatusRequest).await?;
            io.recv_packet::<StatusResponse>().await?;
            anyhow::Ok(())
        };
        tokio::time::timeout(Duration::from_secs(10), exchange)
            .await
            .unwrap()
            .is_ok()
    })
}

#[test]
fn pxp_con_002_a_trusted_upstream_is_read_after_its_header() {
    let (_app, address) = server(Ipv4Addr::LOCALHOST.into());
    assert!(answered(address, &v1("203.0.113.7")), "v1");
    assert!(answered(address, V2_LOCAL), "v2 LOCAL");
    assert!(!answered(address, b""), "no header");
    assert!(
        !answered(
            address,
            b"PROXY TCP4 203.0.113.07 192.0.2.10 51234 25565\r\n"
        ),
        "malformed header"
    );
}

#[test]
fn pxp_con_001_a_header_from_another_address_is_a_malformed_handshake() {
    let (_app, address) = server(Ipv4Addr::new(192, 0, 2, 1).into());
    assert!(!answered(address, &v1("203.0.113.7")));
    assert!(answered(address, b""));
}

#[test]
fn pxp_use_002_the_per_ip_limit_charges_the_client_address_of_the_header() {
    let (_app, address) = server(Ipv4Addr::LOCALHOST.into());
    assert!(
        (1..=20).all(|n| answered(address, &v1(&format!("203.0.113.{n}")))),
        "the upstream itself is not charged"
    );
    assert!(
        !(0..20).all(|_| answered(address, &v1("203.0.113.100"))),
        "one client address is charged"
    );
}
