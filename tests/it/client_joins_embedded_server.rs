use bevy_app::{App, AppExit, Update};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Local, Res};
use mcrs_minecraft_client::columns::{BlockSource, ColumnCachePlugin, ColumnStore};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, ClientNetworkPlugin, CurrentDimension, JoinedGame,
    PendingTeleports, ServerProfile, offline_player_uuid,
};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugin, run_server_loop};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::support::{
    JOIN_TIMEOUT, drive_client_until_joined, insert_block_catalog, insert_session_inputs,
};

#[test]
fn the_client_logs_in_configures_and_joins_the_embedded_server() {
    let mut server = App::new();
    server.add_plugins(MinecraftServerPlugin::embedded());
    let address = server.world().resource::<BoundAddress>().0;
    assert!(address.ip().is_loopback(), "embedded server left loopback");

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    server.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stop_flag.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let (synced_send, synced_recv) = mpsc::channel();
    server.add_systems(
        Update,
        move |set: Option<Res<RegistrySet>>, mut sent: Local<bool>| {
            let Some(set) = set.filter(|_| !*sent) else {
                return;
            };
            *sent = true;
            let synced: Vec<(String, Vec<String>)> = set
                .synced()
                .map(|(table, _)| {
                    let names = table.names().iter().map(ToString::to_string);
                    (table.registry().to_string(), names.collect())
                })
                .collect();
            synced_send.send(synced).ok();
        },
    );
    let (finished_send, finished_recv) = mpsc::channel();
    let server_thread = mcrs_minecraft_server::spawn_server_thread(server, move |app| {
        run_server_loop(app);
        finished_send.send(()).ok();
    });

    let mut client = App::new();
    client.add_plugins(ClientNetworkPlugin {
        server: address,
        username: "mcrs_test".to_owned(),
        profile_id: None,
        view_distance: 8,
    });
    client.add_plugins(ColumnCachePlugin);
    insert_block_catalog(&mut client);
    insert_session_inputs(&mut client);

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

    assert_eq!(
        *world.get::<ConnectionState>(connection).unwrap(),
        ConnectionState::Game
    );

    let synced = synced_recv
        .try_recv()
        .expect("the server read its synced registries once its set existed");
    assert!(!synced.is_empty(), "the server syncs no registry");
    let session = world.resource::<RegistrySet>();
    let statics = mcrs_minecraft_world::registries::static_registries().unwrap();
    let received: BTreeSet<String> = session
        .tables()
        .map(|table| table.registry().to_string())
        .filter(|registry| statics.table(registry).is_none())
        .collect();
    let expected: BTreeSet<String> = synced
        .iter()
        .map(|(registry, _)| registry.clone())
        .collect();
    assert_eq!(
        received, expected,
        "the registries the session holds beside the statics"
    );
    for (registry, names) in &synced {
        let held: Vec<String> = session
            .table(registry)
            .unwrap()
            .names()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(&held, names, "entries of {registry}");
    }
    let biomes = synced
        .iter()
        .find(|(registry, _)| registry == "minecraft:worldgen/biome")
        .expect("the server syncs biomes");
    assert!(!biomes.1.is_empty(), "the biome registry arrived empty");

    let joined = world.get::<JoinedGame>(connection).unwrap();
    assert!(!joined.dimensions.is_empty());
    let current = world.get::<CurrentDimension>(connection).unwrap();
    assert!(current.key.as_str().starts_with("minecraft:"));

    assert!(
        world
            .get::<PendingTeleports>(connection)
            .is_some_and(|teleports| !teleports.0.is_empty()),
        "no teleport arrived for the client to confirm"
    );
    assert!(world.get::<ChunkCacheCenter>(connection).is_some());
    assert!(world.get::<ChunkCacheRadius>(connection).is_some());
    let store = world.resource::<ColumnStore>();
    let extent = store
        .extent()
        .expect("the login named a dimension type with a height");
    assert!(!store.is_empty(), "no column reached the store");
    assert!(extent.sections > 0, "a dimension of no sections");
}
