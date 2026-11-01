use bevy_app::{App, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_engine_server::{BoundAddress, DimensionWorldsPlugin, ShutdownRequest};
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_save::{WorldGenSettings, player_dat_path, write_world_gen_settings};
use mcrs_minecraft_server::MinecraftServerPlugins;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::support::{JOIN_TIMEOUT, drive_client_until_joined, joining_client};

fn scratch_world() -> PathBuf {
    let world = std::env::temp_dir().join(format!("mcrs-shutdown-{}", std::process::id()));
    std::fs::create_dir_all(&world).unwrap();
    let presets = test_registries().registry::<WorldPreset>().unwrap();
    let normal = presets
        .by_name(&ResourceLocationBuf::read("minecraft:normal").unwrap())
        .and_then(|id| presets.get(id))
        .unwrap();
    write_world_gen_settings(
        &world,
        &WorldGenSettings {
            seed: 0,
            dimensions: normal.dimensions.clone(),
        },
        test_registries(),
    )
    .unwrap();
    world
}

#[test]
fn a_shutdown_request_saves_the_player_and_the_server_exits_by_itself() {
    let world = scratch_world();
    let mut server = App::new();
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.worlds = DimensionWorldsPlugin {
        world: Some(world.clone()),
        ..Default::default()
    };
    server.add_plugins(plugins);
    let address = server.world().resource::<BoundAddress>().0;

    let requested = Arc::new(AtomicBool::new(false));
    let request = requested.clone();
    server.add_systems(
        Update,
        move |mut requests: MessageWriter<ShutdownRequest>| {
            if request.load(Ordering::Relaxed) {
                requests.write(ShutdownRequest);
            }
        },
    );
    let (exited_send, exited) = mpsc::channel();
    let server_thread = mcrs_minecraft_engine_server::spawn_server_thread(server, move |app| {
        exited_send
            .send(mcrs_minecraft_engine_server::run_server_loop(app))
            .ok();
    })
    .unwrap();

    let mut client = joining_client(address, 2);
    let joined = drive_client_until_joined(&mut client, false);
    assert!(
        joined.is_some(),
        "the client never reached the play state within {JOIN_TIMEOUT:?}"
    );
    let player_file = player_dat_path(&world, offline_player_uuid("mcrs_test"));
    assert!(!player_file.exists(), "saved before the shutdown");

    requested.store(true, Ordering::Relaxed);
    exited
        .recv_timeout(Duration::from_secs(30))
        .expect("the server exits by itself after the request");
    server_thread.join().unwrap();

    assert!(player_file.exists(), "{} is missing", player_file.display());
    std::fs::remove_dir_all(world).unwrap();
}
