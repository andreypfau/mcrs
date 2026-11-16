use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::ServerNetworkPlugin;
use mcrs_minecraft_engine_server::login::JoinRate;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::uuid::Uuid;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::server::RunningServer;

const SEVEN: &str = "203.0.113.7";
const EIGHT: &str = "203.0.113.8";

/// Joins as `name`, from `source` behind a PROXY v1 header when one is given, and answers the
/// refusal as text.
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

fn behind_proxy() -> ServerNetworkPlugin {
    ServerNetworkPlugin {
        proxy_upstreams: vec![Ipv4Addr::LOCALHOST.into()],
        ..ServerNetworkPlugin::embedded()
    }
}

#[test]
fn lim_full_001_a_full_server_refuses_a_new_player_but_not_a_rejoin() {
    let network = ServerNetworkPlugin {
        max_players: 1,
        ..ServerNetworkPlugin::embedded()
    };
    with_server(network, async |address| {
        let _a = join_from(address, "a", None).await.unwrap();
        let reason = refusal(join_from(address, "b", None).await);
        assert!(
            reason.contains(r#"{"translate":"multiplayer.disconnect.server_full"}"#),
            "F-1: {reason}"
        );
        join_from(address, "a", None).await.expect("F-3");
    });
}

#[test]
fn lim_ip_001_one_address_joins_up_to_the_cap() {
    let network = ServerNetworkPlugin {
        per_ip_join_cap: Some(2),
        join_rate: None,
        ..behind_proxy()
    };
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
    let network = ServerNetworkPlugin {
        per_ip_join_cap: None,
        join_rate: Some(JoinRate {
            capacity: 3,
            period: Duration::from_secs(4),
        }),
        ..behind_proxy()
    };
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
