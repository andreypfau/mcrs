use bevy_app::{App, Update};
use bevy_ecs::prelude::{Local, Res};
use mcrs_minecraft_client::columns::{ColumnStore, Columns};
use mcrs_minecraft_client::joined_game::{CurrentDimension, JoinedGame};
use mcrs_minecraft_engine_server::BoundAddress;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, PendingTeleports, ServerProfile, offline_player_uuid,
};
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_world::registry_load::sent_by;
use std::collections::BTreeMap;
use std::sync::mpsc;

use crate::support::{
    JOIN_TIMEOUT, assert_session_registries, drive_client_until_joined, joining_client,
    start_server,
};

type Contents = BTreeMap<String, (Vec<String>, Vec<(String, Vec<u16>)>)>;

fn contents(set: &Registries, registries: &[String]) -> Contents {
    registries
        .iter()
        .map(|registry| {
            let key = ResourceLocationBuf::read(registry).unwrap();
            let names = set
                .names_of(&key)
                .map(|names| names.iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let tags = set
                .tags_of(key.as_location())
                .map(|tags| {
                    tags.map(|(name, members)| (name.to_string(), members.to_vec()))
                        .collect()
                })
                .unwrap_or_default();
            (registry.clone(), (names, tags))
        })
        .collect()
}

#[test]
fn the_client_logs_in_configures_and_joins_the_embedded_server() {
    let mut server = App::new();
    server.add_plugins(MinecraftServerPlugins::embedded());
    let address = server.world().resource::<BoundAddress>().0;
    assert!(address.ip().is_loopback(), "embedded server left loopback");

    let (synced_send, synced_recv) = mpsc::channel();
    server.add_systems(
        Update,
        move |registries: Option<Res<CurrentRegistries>>, mut sent: Local<bool>| {
            let Some(registries) = registries else {
                return;
            };
            if std::mem::replace(&mut *sent, true) {
                return;
            }
            let received = sent_by(&registries, false);
            let named: Vec<String> = received
                .registries
                .iter()
                .map(|(registry, _)| registry.to_string())
                .chain(
                    received
                        .tags
                        .iter()
                        .map(|(registry, _)| registry.to_string()),
                )
                .collect();
            let synced = received.registries.len();
            synced_send
                .send((synced, contents(&registries, &named), named))
                .ok();
        },
    );
    let stop_server = start_server(server);

    let mut client = joining_client(address, 8);
    let outcome = drive_client_until_joined(&mut client, true);
    stop_server();

    let Some(connection) = outcome else {
        panic!("the client never reached the play state within {JOIN_TIMEOUT:?}");
    };

    let world = client.world();
    let profile = world.get::<ServerProfile>(connection).unwrap();
    assert_eq!(profile.username, "mcrs_test");
    assert_eq!(profile.id, offline_player_uuid("mcrs_test"));

    let (synced, server, named) = synced_recv
        .try_recv()
        .expect("the server read its registries once they existed");
    assert!(synced > 0, "the server syncs no registry");
    assert!(
        named
            .iter()
            .any(|registry| registry == "minecraft:worldgen/biome"),
        "the server syncs biomes"
    );
    assert_eq!(
        contents(world.resource::<CurrentRegistries>(), &named),
        server,
        "the client holds the names, id order and tags the server holds"
    );

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

    assert_session_registries(&client, connection);
}
