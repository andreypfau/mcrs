use crate::configuration_registries::ASSETS;
use bevy_app::{App, AppExit, PluginGroup, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_core::{ResourceLocation, VERSION};
use mcrs_minecraft_light_plugin::LightingPlugin;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundRegistryData;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugins, run_server_loop};
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Stops the server loop on the way out of the test, a failed one included, so its thread does
/// not keep ticking beside the other tests of the binary.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[test]
fn a_client_without_an_app_joins_the_embedded_server_offline() {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network.announce_on_lan = false;
    let mut app = App::new();
    app.add_plugins(
        plugins
            .build()
            .disable::<WorldGenPlugin>()
            .disable::<LightingPlugin>(),
    );
    let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
    let stopped = stop.0.clone();
    app.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stopped.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let address = app.world().resource::<BoundAddress>().0;
    let (finished_send, finished_recv) = std::sync::mpsc::channel();
    let server = mcrs_minecraft_server::spawn_server_thread(app, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    })
    .expect("the server thread starts");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (joined, first_game_packet) = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let io = PacketIo::connect(address).await.unwrap();
            let Joined {
                mut connection,
                profile,
                configuration,
            } = join(
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
            let first = loop {
                if let Some(packet) = connection.try_recv().unwrap() {
                    break packet.id;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            ((profile, configuration), first)
        })
        .await
        .expect("the join and the first game packet within the timeout")
    });
    drop(stop);
    finished_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the embedded server did not stop after AppExit");
    server.join().unwrap();

    let (profile, configuration) = joined;
    assert_eq!(profile.username, "Bot");
    let dimension_type = ResourceLocation::read("minecraft:dimension_type").unwrap();
    assert!(
        configuration.registry_data.iter().any(|body| {
            let data = ClientboundRegistryData::decode(&mut &body[..]).unwrap();
            data.registry == dimension_type && !data.entries.is_empty()
        }),
        "the server sent its dimension types"
    );
    assert_eq!(first_game_packet, ClientboundLogin::ID);
}
