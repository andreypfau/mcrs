use bevy_app::App;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{BoundAddress, NetworkPlugin, StatusPlayers, StatusPublisher};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

#[derive(Deserialize)]
struct Status {
    players: Players,
    description: Option<String>,
}

#[derive(Deserialize)]
struct Players {
    max: u32,
    online: u32,
    sample: Option<Vec<Sample>>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Sample {
    id: Uuid,
    name: String,
}

fn server(max_players: u32, motd: &str, players: Vec<(Uuid, String)>) -> (App, SocketAddr) {
    server_with(
        NetworkPlugin {
            max_players,
            motd: motd.to_owned(),
            ..Default::default()
        },
        players,
    )
}

fn server_with(plugin: NetworkPlugin, players: Vec<(Uuid, String)>) -> (App, SocketAddr) {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
        announce_on_lan: false,
        ..plugin
    });
    app.world_mut()
        .resource_mut::<StatusPublisher>()
        .publish(StatusPlayers { players });
    let address = app.world().resource::<BoundAddress>().0;
    app.update();
    (app, address)
}

fn status_json(address: SocketAddr) -> String {
    status_exchange(address).unwrap()
}

fn status_exchange(address: SocketAddr) -> anyhow::Result<String> {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut io = PacketIo::connect(address).await?;
            io.send_packet(&ServerboundHandshake {
                protocol_version: VarInt(VERSION.protocol_version),
                server_address: Bounded("127.0.0.1"),
                server_port: address.port(),
                intent: Intent::Status,
            })
            .await?;
            io.send_packet(&StatusRequest).await?;
            Ok(io.recv_packet::<StatusResponse>().await?.json.to_owned())
        })
        .await
        .unwrap()
    })
}

#[test]
fn a_player_who_joins_shows_in_the_next_status_response() {
    let players = (1..=3)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    let (mut app, address) = server(7, "§aTest", players);

    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();
    assert_eq!(status.players.online, 3);
    let more = (1..=4)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    app.world_mut()
        .resource_mut::<StatusPublisher>()
        .publish(StatusPlayers { players: more });

    app.update();
    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();

    assert_eq!(status.players.online, 4);
}

#[test]
fn the_status_response_stays_the_same_while_no_one_joins() {
    let players = (1..=17)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    let (mut app, address) = server(7, "§aTest", players);

    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();

    app.update();

    let json = status_json(address);
    let status2: Status = serde_json::from_str(&json).unwrap();

    assert_eq!(status2.players.sample, status.players.sample);
}

#[test]
fn the_status_response_shows_the_settings_and_the_online_players() {
    let players = (1..=3)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    let (_app, address) = server(7, "§aTest", players);

    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();

    assert_eq!(status.players.max, 7);
    assert_eq!(status.players.online, 3);
    assert_eq!(status.description.as_deref(), Some("§aTest"));
    let mut sample = status.players.sample.unwrap();
    sample.sort();
    let expected: Vec<_> = (1..=3)
        .map(|n| Sample {
            id: Uuid::from_u128(n),
            name: format!("player{n}"),
        })
        .collect();
    assert_eq!(sample, expected);
    assert!(
        json.contains(r#"{"id":"00000000-0000-0000-0000-000000000001","name":"player1"}"#),
        "{json}"
    );
}

#[test]
fn the_enable_status_disable() {
    let (_app, address) = server_with(
        NetworkPlugin {
            enable_status: false,
            ..Default::default()
        },
        vec![],
    );
    let status = status_exchange(address);
    assert!(status.is_err(), "{status:?}")
}

#[test]
fn the_sample_of_enable_hide_online_players() {
    let players = (1..=3)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    let (_app, address) = server_with(
        NetworkPlugin {
            hide_online_players: true,
            ..Default::default()
        },
        players,
    );
    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();

    assert!(status.players.sample.is_none(), "{json}");
    assert_eq!(status.players.online, 3);
}

#[test]
fn the_sample_of_more_than_twelve_players_is_twelve_adjacent_ones() {
    let players = (0..15)
        .map(|n| (Uuid::from_u128(n), format!("player{n}")))
        .collect();
    let (_app, address) = server(20, "mcrs", players);

    let status: Status = serde_json::from_str(&status_json(address)).unwrap();

    assert_eq!(status.players.online, 15);
    let mut positions: Vec<u128> = status
        .players
        .sample
        .unwrap()
        .iter()
        .map(|sample| sample.id.as_u128())
        .collect();
    positions.sort_unstable();
    let start = *positions.first().unwrap();
    assert!(start <= 3, "{positions:?}");
    assert_eq!(positions, (start..start + 12).collect::<Vec<_>>());
}

#[test]
fn an_empty_server_with_no_motd_sends_neither_sample_nor_description() {
    let (_app, address) = server(20, "", Vec::new());

    let json = status_json(address);
    let status: Status = serde_json::from_str(&json).unwrap();

    assert_eq!(status.players.online, 0);
    assert!(status.players.sample.is_none(), "{json}");
    assert!(status.description.is_none(), "{json}");
}
