use bevy_app::{App, AppExit, Update};
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, ClientNetworkPlugin, JoinedGame, ReceivedChunkColumns,
    ReceivedRegistries, ReceivedTags, ServerPosition, ServerProfile, offline_player_uuid,
};
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugin, run_server_loop};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const JOIN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn the_client_logs_in_configures_and_joins_the_embedded_server() {
    let mut server = App::new();
    server.add_plugins(MinecraftServerPlugin::embedded());
    let address = server.world().resource::<BoundAddress>().0;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    server.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stop_flag.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let (finished_send, finished_recv) = mpsc::channel();
    let server_thread = mcrs_minecraft_server::spawn_server_thread(server, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    });

    let mut client = App::new();
    client.add_plugins(ClientNetworkPlugin {
        server: address,
        username: "mcrs_test".to_owned(),
    });

    let outcome = drive_client_until_joined(&mut client);

    stop.store(true, Ordering::Relaxed);
    finished_recv
        .recv_timeout(Duration::from_secs(30))
        .expect("the embedded server did not stop after AppExit");
    server_thread.join().expect("server thread joined");

    let Some(connection) = outcome else {
        panic!("the client never reached the play state within {JOIN_TIMEOUT:?}");
    };

    let world = client.world();
    let profile = world.get::<ServerProfile>(connection).unwrap();
    assert_eq!(profile.username, "mcrs_test");
    assert_eq!(profile.id, offline_player_uuid("mcrs_test"));

    assert_eq!(*world.get::<ConnectionState>(connection).unwrap(), ConnectionState::Game);

    let registries = world.get::<ReceivedRegistries>(connection).unwrap();
    assert_eq!(registries.0.len(), 29, "registry packet count");
    assert!(
        registries
            .0
            .iter()
            .any(|r| r.registry == "minecraft:worldgen/biome" && !r.entries.is_empty()),
        "the biome registry arrived empty"
    );

    let tags = world.get::<ReceivedTags>(connection).unwrap();
    assert!(!tags.0.is_empty(), "no tags arrived");

    let joined = world.get::<JoinedGame>(connection).unwrap();
    assert!(!joined.dimensions.is_empty());
    assert!(joined.dimension.starts_with("minecraft:"));

    assert!(world.get::<ServerPosition>(connection).is_some());
    assert!(world.get::<ChunkCacheCenter>(connection).is_some());
    assert!(world.get::<ChunkCacheRadius>(connection).is_some());
    assert!(world.get::<ReceivedChunkColumns>(connection).unwrap().0 > 0);
}

/// Returns the connection entity once every play-state packet the flow promises
/// has arrived, or `None` if the deadline passes first.
fn drive_client_until_joined(client: &mut App) -> Option<bevy_ecs::entity::Entity> {
    let deadline = Instant::now() + JOIN_TIMEOUT;
    let mut seen_configuration = false;

    while Instant::now() < deadline {
        client.update();

        let world = client.world_mut();
        let mut connections = world.query::<(bevy_ecs::entity::Entity, &ConnectionState)>();
        let Some((entity, state)) = connections.iter(world).next() else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        seen_configuration |= *state == ConnectionState::Configuration;
        if *state != ConnectionState::Game {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        assert!(
            seen_configuration,
            "the connection reached play without passing through configuration"
        );
        let joined = world.get::<JoinedGame>(entity).is_some();
        let positioned = world.get::<ServerPosition>(entity).is_some();
        let centred = world.get::<ChunkCacheCenter>(entity).is_some();
        let radius = world.get::<ChunkCacheRadius>(entity).is_some();
        let chunks = world
            .get::<ReceivedChunkColumns>(entity)
            .is_some_and(|c| c.0 > 0);
        if joined && positioned && centred && radius && chunks {
            return Some(entity);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}
