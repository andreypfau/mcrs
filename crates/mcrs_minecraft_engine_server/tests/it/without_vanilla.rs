use bevy_app::{App, AppExit, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{
    BoundAddress, DataPack, DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin,
    run_server_loop, spawn_server_thread,
};
use mcrs_minecraft_network::join::{Join, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol::uuid::Uuid;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

/// Stops the server loop on the way out of the test, a failed one included.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// The engine's own server, with no world generation, light, command or chat plugin, lets a
/// client into a world with no terrain.
#[test]
fn a_client_joins_the_engine_server_without_vanilla_plugins() {
    let mut app = App::new();
    app.add_plugins((
        ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(ASSETS.to_owned()),
            data_pack: DataPack::Directory(mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
            ..Default::default()
        },
        DimensionWorldsPlugin::default(),
        ServerNetworkPlugin {
            announce_on_lan: false,
            ..ServerNetworkPlugin::embedded()
        },
    ));
    let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
    let stopped = stop.0.clone();
    app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stopped.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let address = app.world().resource::<BoundAddress>().0;
    let (finished_send, finished_recv) = std::sync::mpsc::channel();
    let server = spawn_server_thread(app, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    })
    .expect("the server thread starts");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let first_game_packet = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let io = PacketIo::connect(address).await.unwrap();
            let mut joined = join(
                io,
                Join {
                    server_address: address.ip().to_string(),
                    server_port: address.port(),
                    remote_addr: address,
                    username: "Bot".to_owned(),
                    profile_id: Uuid::nil(),
                    protocol_version: VERSION.protocol_version,
                    view_distance: 2,
                    holds_vanilla_pack: false,
                },
            )
            .await
            .unwrap();
            loop {
                if let Some(packet) = joined.connection.try_recv().unwrap() {
                    break packet.id;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the join and the first game packet within the timeout")
    });
    drop(stop);
    finished_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the server did not stop after AppExit");
    server.join().unwrap();

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
