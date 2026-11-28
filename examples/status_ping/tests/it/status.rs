use bevy_app::App;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::{BoundAddress, NetworkPlugin, StatusPlayers, StatusPublisher};
use mcrs_minecraft_protocol::uuid::Uuid;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::Command;

#[test]
fn status_seq_001_prints_the_embedded_servers_status_and_latency() {
    for motd in ["Hello §aMinecraft\nsecond line", ""] {
        let mut app = App::new();
        app.add_plugins(NetworkPlugin {
            address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            announce_on_lan: false,
            motd: motd.to_owned(),
            max_players: 7,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<StatusPublisher>()
            .publish(StatusPlayers {
                players: vec![(Uuid::from_u128(1), "visitor".to_owned())],
            });
        app.update();
        let address = app.world().resource::<BoundAddress>().0;

        let output = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_status_ping"))
            .args(["127.0.0.1", &address.port().to_string()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let expected = format!(
            "Version: {} (protocol {})\nDescription: {motd}\nPlayers: 1/7\nLatency: ",
            VERSION.name, VERSION.protocol_version,
        );
        let latency = stdout.strip_prefix(&expected).unwrap();
        let millis: f64 = latency.trim().strip_suffix(" ms").unwrap().parse().unwrap();
        assert!(
            millis.is_finite() && millis >= 0.0,
            "latency in milliseconds: {latency}"
        );
    }
}
