use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::ServerNetworkPlugin;
use mcrs_minecraft_engine_server::login::{JoinLimits, JoinRate};
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::server::RunningServer;

const SEVEN: &str = "203.0.113.7";
const EIGHT: &str = "203.0.113.8";

async fn join_from(
    address: SocketAddr,
    name: &'static str,
    source: Option<&'static str>,
) -> Result<Joined, String> {
    let mut stream = TcpStream::connect(address).await.unwrap();
    if let Some(source) = source {
        let header = format!("PROXY TCP4 {source} 192.0.2.10 51234 25565\r\n");
        stream.write_all(header.as_bytes()).await.unwrap();
    }
    let joined = join(
        PacketIo::new(stream),
        Join {
            server_address: address.ip().to_string(),
            server_port: address.port(),
            remote_addr: address,
            username: name.to_owned(),
            profile_id: Uuid::nil(),
            protocol_version: VERSION.protocol_version,
            view_distance: 2,
            holds_vanilla_pack: false,
        },
    )
    .await;
    joined.map_err(|error| format!("{error:#}"))
}

async fn in_play(joined: &mut Joined) {
    loop {
        if let Some(packet) = joined.connection.try_recv().unwrap()
            && packet.id == ClientboundLogin::ID
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn limits(limits: JoinLimits) -> ServerNetworkPlugin {
    ServerNetworkPlugin {
        join_limits: limits,
        ..ServerNetworkPlugin::embedded()
    }
}

fn refusal(joined: Result<Joined, String>) -> String {
    joined.err().expect("the login is refused")
}

fn with_server(network: ServerNetworkPlugin, test: impl AsyncFnOnce(SocketAddr)) {
    let server = RunningServer::start(network);
    let address = server.address;
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(60), test(address))
            .await
            .expect("the logins end within the timeout");
    });
    server.stop();
}

fn behind_proxy(join_limits: JoinLimits) -> ServerNetworkPlugin {
    ServerNetworkPlugin {
        proxy_upstreams: vec![Ipv4Addr::LOCALHOST.into()],
        ..limits(join_limits)
    }
}

const SERVER_FULL: &str = r#"{"translate":"multiplayer.disconnect.server_full"}"#;

#[test]
fn lim_full_001_a_full_server_refuses_a_new_player_but_not_a_rejoin() {
    let network = limits(JoinLimits {
        max_players: 1,
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let mut a = join_from(address, "a", None).await.unwrap();
        in_play(&mut a).await;
        let reason = refusal(join_from(address, "b", None).await);
        assert!(reason.contains(SERVER_FULL), "F-1: {reason}");
        join_from(address, "a", None).await.expect("F-3");
    });
}

#[test]
fn lim_full_001_a_slot_comes_free_when_its_player_leaves() {
    let network = limits(JoinLimits {
        max_players: 1,
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let mut a = join_from(address, "a", None).await.unwrap();
        in_play(&mut a).await;
        drop(a);
        loop {
            match join_from(address, "b", None).await {
                Ok(_) => break,
                Err(reason) => assert!(reason.contains(SERVER_FULL), "F-4: {reason}"),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}

#[test]
fn lim_full_001_a_player_limit_of_zero_refuses_every_login() {
    let network = limits(JoinLimits {
        max_players: 0,
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let reason = refusal(join_from(address, "a", None).await);
        assert!(reason.contains(SERVER_FULL), "F-5: {reason}");
    });
}

#[test]
fn lim_ip_001_loopback_joins_past_the_cap_and_the_join_rate() {
    let network = limits(JoinLimits {
        per_ip: Some(1),
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let mut players = Vec::new();
        for name in ["a", "b", "c", "d", "e", "f"] {
            players.push(join_from(address, name, None).await.expect("I-5"));
        }
    });
}

#[test]
fn lim_ip_001_one_address_joins_up_to_the_cap() {
    let network = behind_proxy(JoinLimits {
        per_ip: Some(2),
        join_rate: None,
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let _a = join_from(address, "a", Some(SEVEN)).await.unwrap();
        let _b = join_from(address, "b", Some(SEVEN)).await.unwrap();
        let reason = refusal(join_from(address, "c", Some(SEVEN)).await);
        assert!(
            reason.contains(r#""Too many connections from your address""#),
            "I-1: {reason}"
        );
        join_from(address, "d", Some(EIGHT)).await.expect("I-2");
        join_from(address, "a", Some(SEVEN)).await.expect("I-4");
    });
}

#[test]
fn lim_rate_001_a_burst_of_logins_from_one_address_empties_its_bucket() {
    let network = behind_proxy(JoinLimits {
        per_ip: None,
        join_rate: Some(JoinRate {
            capacity: 3,
            period: Duration::from_secs(3600),
        }),
        ..JoinLimits::default()
    });
    with_server(network, async |address| {
        let burst: Vec<_> = ["a", "b", "c", "d"]
            .into_iter()
            .map(|name| tokio::spawn(join_from(address, name, Some(SEVEN))))
            .collect();
        let mut joined = Vec::new();
        let mut refused = Vec::new();
        for login in burst {
            match login.await.unwrap() {
                Ok(player) => joined.push(player),
                Err(reason) => refused.push(reason),
            }
        }
        assert_eq!(joined.len(), 3, "R-1: {refused:?}");
        assert!(
            refused[0].contains(
                r#""Too many logins from your address. Wait a few seconds and try again.""#
            ),
            "R-1: {refused:?}"
        );
        join_from(address, "e", Some(EIGHT)).await.expect("R-3");
    });
}
