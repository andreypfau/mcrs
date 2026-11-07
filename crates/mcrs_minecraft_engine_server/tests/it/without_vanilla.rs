use mcrs_minecraft_engine_server::ServerNetworkPlugin;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use std::process::Command;
use std::time::Duration;

use crate::server::RunningServer;

/// The engine's own server, with no world generation, light, command or chat plugin, lets a
/// client into a world with no terrain.
#[test]
fn a_client_joins_the_engine_server_without_vanilla_plugins() {
    let server = RunningServer::start(ServerNetworkPlugin::embedded());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let first_game_packet = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let mut connection = server.join().await;
            loop {
                if let Some(packet) = connection.try_recv().unwrap() {
                    break packet.id;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the join and the first game packet within the timeout")
    });
    server.stop();

    assert_eq!(first_game_packet, ClientboundLogin::ID);
}

/// A program that builds the engine's server does not link world generation, Anvil or light.
#[test]
fn the_engine_server_links_no_world_generation_anvil_or_light_crate() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "--locked",
            "--package",
            env!("CARGO_PKG_NAME"),
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).unwrap();
    let forbidden: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|&name| {
            let world_generation = name.starts_with("mcrs_minecraft_worldgen");
            let vanilla_plugin = name.starts_with("mcrs_minecraft_vanilla_");
            world_generation
                || vanilla_plugin
                || name == "mcrs_minecraft_anvil"
                || name == "mcrs_minecraft_light_engine"
        })
        .collect();
    assert!(
        forbidden.is_empty(),
        "the engine server links {forbidden:?}"
    );
}
